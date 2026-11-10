//! Recent files: images viewed in Specular go into the desktop's shared recent list,
//! `$XDG_DATA_HOME/recently-used.xbel` (the freedesktop "desktop bookmarks" file GTK's
//! `GtkRecentManager` and KDE's `KRecentDocument` read: GNOME Files "Recent", Dolphin
//! "Recent Files", file choosers, launchers' recent menus).
//!
//! Applying only records that the option is on (and backs the file up); each viewed file then
//! adds or refreshes one `<bookmark>` with a `<bookmark:application name="Specular">`
//! ([`note`]). Removing takes out every Specular application entry and the bookmarks that
//! had no other application, so the list is as other programs left it.
//!
//! The file is shared with every GTK and KDE program, so it is edited as text, touching only
//! our own elements, and written atomically.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use specular_log::{SETTINGS, info};

use super::{Record, backup, write_atomic};
use crate::integration::{Env, Error, OptionId, Result, State};

/// The `name` of our `<bookmark:application>` entries.
pub const APP_NAME: &str = "Specular";
/// Bookmarks only Specular holds, at most (the oldest go first).
const MAX_OURS: usize = 200;

pub fn xbel_path(env: &Env) -> PathBuf {
    env.data_home.join("recently-used.xbel")
}

/// Whether the option is on.
pub fn enabled(env: &Env) -> bool {
    Record::load(env, OptionId::RecentFiles.key()).is_some()
}

pub(super) fn apply(env: &Env) -> Result<()> {
    let key = OptionId::RecentFiles.key();
    let mut rec = Record::load(env, key).unwrap_or_default();
    if !rec
        .rows
        .iter()
        .any(|r| r.first().is_some_and(|c| c == "on"))
    {
        // The list as it was before Specular wrote to it.
        backup(env, &xbel_path(env))?;
        rec.rows.push(vec!["on".into()]);
    }
    rec.save(env, key)
}

pub(super) fn remove(env: &Env) -> Result<()> {
    let path = xbel_path(env);
    if let Ok(text) = fs::read_to_string(&path) {
        let (cleaned, dropped) = without_ours(&text);
        if cleaned != text {
            backup(env, &path)?;
            write_atomic(&path, cleaned.as_bytes(), mode_of(&path))?;
            info!(target: SETTINGS, "integration: took Specular's entries out of {} ({dropped} bookmarks removed)", path.display());
        }
    }
    Record::delete(env, OptionId::RecentFiles.key());
    Ok(())
}

pub(super) fn status(env: &Env) -> (State, Vec<String>) {
    if !enabled(env) {
        return (State::Off, Vec::new());
    }
    let n = fs::read_to_string(xbel_path(env))
        .map(|t| bookmarks(&t).filter(|b| has_ours(b.text)).count())
        .unwrap_or(0);
    (State::On, vec![n.to_string()])
}

fn mode_of(p: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(p).map_or(0o600, |m| m.permissions().mode() & 0o7777)
}

/// Adds `file` (an image just viewed, of type `mime`) to the recent list, or refreshes its
/// entry. Does nothing while the option is off.
pub fn note(env: &Env, file: &Path, mime: &str) -> Result<()> {
    if !enabled(env) || !file.is_absolute() {
        return Ok(());
    }
    let path = xbel_path(env);
    let text = match fs::read(&path) {
        Ok(b) => String::from_utf8(b)
            .map_err(|_| Error::Unrecognized(path.clone(), "not UTF-8".into()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(Error::Io(path, e)),
    };
    let href = crate::launch::file_uri(file);
    let exec = format!("'{} %u'", super::shell_quote(&env.exe));
    let new = noted(&text, &href, mime, &exec, &now_stamp())
        .ok_or_else(|| Error::Unrecognized(path.clone(), "not a desktop bookmark file".into()))?;
    if new != text {
        write_atomic(&path, new.as_bytes(), mode_of(&path))?;
    }
    Ok(())
}

// ----- Editing ----------------------------------------------------------------------------

/// One `<bookmark …>…</bookmark>` element of the file.
struct Bookmark<'a> {
    start: usize,
    end: usize,
    text: &'a str,
}

impl Bookmark<'_> {
    fn href(&self) -> Option<String> {
        let open = &self.text[..self.text.find('>')?];
        attr(open, "href").map(|v| unescape(&v))
    }
}

/// The bookmark elements, in file order.
fn bookmarks(text: &str) -> impl Iterator<Item = Bookmark<'_>> {
    let mut at = 0;
    std::iter::from_fn(move || {
        loop {
            let rel = text[at..].find("<bookmark")?;
            let start = at + rel;
            let after = text[start + "<bookmark".len()..].chars().next()?;
            if !after.is_whitespace() {
                // `<bookmark:applications>` and friends.
                at = start + 1;
                continue;
            }
            let open_end = start + text[start..].find('>')?;
            let end = if text[..open_end].ends_with('/') {
                open_end + 1
            } else {
                open_end + text[open_end..].find("</bookmark>")? + "</bookmark>".len()
            };
            at = end;
            return Some(Bookmark {
                start,
                end,
                text: &text[start..end],
            });
        }
    })
}

/// The value of attribute `name` in an opening tag (raw, still escaped).
fn attr(tag: &str, name: &str) -> Option<String> {
    let mut rest = tag;
    loop {
        let i = rest.find(name)?;
        let before = rest[..i].chars().last();
        let after = &rest[i + name.len()..];
        if before.is_some_and(char::is_whitespace)
            && let Some(v) = after.trim_start().strip_prefix('=')
        {
            let v = v.trim_start();
            let q = v.chars().next()?;
            if q == '"' || q == '\'' {
                let body = &v[1..];
                return Some(body[..body.find(q)?].to_owned());
            }
        }
        rest = &rest[i + name.len()..];
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Whether a bookmark lists Specular among its applications.
fn has_ours(bookmark: &str) -> bool {
    our_app(bookmark).is_some()
}

/// The byte range of our `<bookmark:application …/>` element in a bookmark.
fn our_app(bookmark: &str) -> Option<(usize, usize)> {
    let mut at = 0;
    while let Some(rel) = bookmark[at..].find("<bookmark:application ") {
        let s = at + rel;
        let e = s + bookmark[s..].find("/>")? + 2;
        if attr(&bookmark[s..e], "name").is_some_and(|n| unescape(&n) == APP_NAME) {
            return Some((s, e));
        }
        at = e;
    }
    None
}

/// Number of `<bookmark:application …/>` entries in a bookmark.
fn app_count(bookmark: &str) -> usize {
    bookmark.matches("<bookmark:application ").count()
}

/// The current time as the file writes it (`2026-10-10T12:34:56.000000Z`).
fn now_stamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs()) as i64;
    stamp(secs)
}

fn stamp(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    // Days to civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.000000Z",
        tod / 3600,
        tod / 60 % 60,
        tod % 60
    )
}

const HEADER: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<xbel version=\"1.0\"\n      \
xmlns:bookmark=\"http://www.freedesktop.org/standards/desktop-bookmarks\"\n      \
xmlns:mime=\"http://www.freedesktop.org/standards/shared-mime-info\"\n>\n";

/// `text` with `href` added or refreshed. `None`: `text` is not an XBEL file.
fn noted(text: &str, href: &str, mime: &str, exec: &str, now: &str) -> Option<String> {
    let text = if text.trim().is_empty() {
        format!("{HEADER}</xbel>\n")
    } else {
        text.to_owned()
    };
    let close = text.rfind("</xbel>")?;
    let app = format!(
        "<bookmark:application name=\"{}\" exec=\"{}\" modified=\"{now}\" count=\"{{count}}\"/>",
        escape(APP_NAME),
        escape(exec)
    );
    if let Some(b) = bookmarks(&text).find(|b| b.href().as_deref() == Some(href)) {
        let mut bm = b.text.to_owned();
        // Refresh the times of the bookmark itself.
        let open_end = bm.find('>')?;
        let mut open = bm[..open_end].to_owned();
        for key in ["modified", "visited"] {
            if let Some(v) = attr(&open, key) {
                open = open.replacen(&format!("{key}=\"{v}\""), &format!("{key}=\"{now}\""), 1);
            }
        }
        bm.replace_range(..open_end, &open);
        match our_app(&bm) {
            Some((s, e)) => {
                let count = attr(&bm[s..e], "count")
                    .and_then(|c| c.parse::<u64>().ok())
                    .unwrap_or(0)
                    + 1;
                bm.replace_range(s..e, &app.replace("{count}", &count.to_string()));
            }
            None => {
                // A line of its own before `</bookmark:applications>`, one step further in.
                let i = bm.find("</bookmark:applications>")?;
                let line = bm[..i].rfind('\n').map_or(0, |n| n + 1);
                let indent = if bm[line..i].trim().is_empty() {
                    format!("{}  ", &bm[line..i])
                } else {
                    String::new()
                };
                let at = if indent.is_empty() { i } else { line };
                let tail = if indent.is_empty() { "" } else { "\n" };
                bm.insert_str(
                    at,
                    &format!("{indent}{}{tail}", app.replace("{count}", "1")),
                );
            }
        }
        let mut out = text.clone();
        out.replace_range(b.start..b.end, &bm);
        return Some(out);
    }
    let entry = format!(
        "  <bookmark href=\"{href}\" added=\"{now}\" modified=\"{now}\" visited=\"{now}\">\n    \
         <info>\n      \
         <metadata owner=\"http://freedesktop.org\">\n        \
         <mime:mime-type type=\"{mime}\"/>\n        \
         <bookmark:applications>\n          \
         {app}\n        \
         </bookmark:applications>\n      \
         </metadata>\n    \
         </info>\n  \
         </bookmark>\n",
        href = escape(href),
        mime = escape(mime),
        app = app.replace("{count}", "1"),
    );
    let mut out = text[..close].to_owned();
    out.push_str(&entry);
    out.push_str(&text[close..]);
    Some(prune(&out))
}

/// Drops the oldest bookmarks only Specular holds beyond [`MAX_OURS`].
fn prune(text: &str) -> String {
    let ours: Vec<(usize, usize)> = bookmarks(text)
        .filter(|b| has_ours(b.text) && app_count(b.text) == 1)
        .map(|b| (b.start, b.end))
        .collect();
    if ours.len() <= MAX_OURS {
        return text.to_owned();
    }
    let drop = &ours[..ours.len() - MAX_OURS];
    cut(text, drop)
}

/// `text` without the byte ranges `ranges` (in order), each with the blank line it leaves.
fn cut(text: &str, ranges: &[(usize, usize)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for &(s, e) in ranges {
        // Take the line's indentation and the line break along.
        let s = text[..s].trim_end_matches([' ', '\t']).len();
        let e = if text[e..].starts_with('\n') {
            e + 1
        } else {
            e
        };
        out.push_str(&text[at..s]);
        at = e;
    }
    out.push_str(&text[at..]);
    out
}

/// `text` without Specular: our application entries, and the bookmarks that only we held.
/// Returns the text and how many bookmarks went.
fn without_ours(text: &str) -> (String, usize) {
    let mut whole = Vec::new();
    let mut entries = Vec::new();
    for b in bookmarks(text) {
        let Some((s, e)) = our_app(b.text) else {
            continue;
        };
        if app_count(b.text) == 1 {
            whole.push((b.start, b.end));
        } else {
            entries.push((b.start + s, b.start + e));
        }
    }
    let mut ranges: Vec<(usize, usize)> = whole.iter().chain(&entries).copied().collect();
    ranges.sort();
    (cut(text, &ranges), whole.len())
}

#[cfg(test)]
mod tests {
    use super::super::test_env;
    use super::*;

    const GTK: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<xbel version=\"1.0\"\n      \
xmlns:bookmark=\"http://www.freedesktop.org/standards/desktop-bookmarks\"\n      \
xmlns:mime=\"http://www.freedesktop.org/standards/shared-mime-info\"\n>\n  \
<bookmark href=\"file:///home/me/notes.txt\" added=\"2026-01-01T10:00:00.000000Z\" modified=\"2026-01-01T10:00:00.000000Z\" visited=\"2026-01-01T10:00:00.000000Z\">\n    \
<info>\n      <metadata owner=\"http://freedesktop.org\">\n        \
<mime:mime-type type=\"text/plain\"/>\n        <bookmark:applications>\n          \
<bookmark:application name=\"Text Editor\" exec=\"&apos;gnome-text-editor %u&apos;\" modified=\"2026-01-01T10:00:00.000000Z\" count=\"1\"/>\n        \
</bookmark:applications>\n      </metadata>\n    </info>\n  </bookmark>\n  \
<bookmark href=\"file:///home/me/a%20b.jpg\" added=\"2026-01-02T10:00:00.000000Z\" modified=\"2026-01-02T10:00:00.000000Z\" visited=\"2026-01-02T10:00:00.000000Z\">\n    \
<info>\n      <metadata owner=\"http://freedesktop.org\">\n        \
<mime:mime-type type=\"image/jpeg\"/>\n        <bookmark:applications>\n          \
<bookmark:application name=\"Image Viewer\" exec=\"&apos;loupe %u&apos;\" modified=\"2026-01-02T10:00:00.000000Z\" count=\"2\"/>\n        \
</bookmark:applications>\n      </metadata>\n    </info>\n  </bookmark>\n\
</xbel>";

    const NOW: &str = "2026-10-10T12:00:00.000000Z";

    #[test]
    fn stamps() {
        assert_eq!(stamp(0), "1970-01-01T00:00:00.000000Z");
        assert_eq!(stamp(1_791_633_600), "2026-10-10T12:00:00.000000Z");
        assert_eq!(stamp(951_782_400), "2000-02-29T00:00:00.000000Z");
    }

    #[test]
    fn add_refresh_and_remove_keep_the_rest() {
        let exec = "'/usr/bin/specular %u'";
        // A new file: a new bookmark, everything else byte for byte.
        let one = noted(GTK, "file:///p/x&y.png", "image/png", exec, NOW).unwrap();
        assert!(one.starts_with(&GTK[..GTK.len() - "</xbel>".len()]));
        assert!(one.contains("href=\"file:///p/x&amp;y.png\""));
        assert!(one.contains("exec=\"&apos;/usr/bin/specular %u&apos;\""));
        assert_eq!(bookmarks(&one).count(), 3);
        // Again: refreshed, not duplicated.
        let two = noted(
            &one,
            "file:///p/x&y.png",
            "image/png",
            exec,
            "2026-10-11T00:00:00.000000Z",
        )
        .unwrap();
        assert_eq!(bookmarks(&two).count(), 3);
        assert!(two.contains("count=\"2\""));
        // A file another program opened: our entry joins theirs.
        let three = noted(&two, "file:///home/me/a%20b.jpg", "image/jpeg", exec, NOW).unwrap();
        assert_eq!(bookmarks(&three).count(), 3);
        let b = bookmarks(&three)
            .find(|b| b.href().as_deref() == Some("file:///home/me/a%20b.jpg"))
            .unwrap();
        assert_eq!(app_count(b.text), 2);
        assert!(b.text.contains(&format!("visited=\"{NOW}\"")));
        // Removing restores the other programs' list exactly, apart from the visit times.
        let (back, dropped) = without_ours(&three);
        assert_eq!(dropped, 1);
        let gtk_now = GTK.replacen(
            "modified=\"2026-01-02T10:00:00.000000Z\" visited=\"2026-01-02T10:00:00.000000Z\"",
            &format!("modified=\"{NOW}\" visited=\"{NOW}\""),
            1,
        );
        assert_eq!(back, gtk_now);
        // A file without our entries is left alone.
        assert_eq!(without_ours(GTK).0, GTK);
    }

    #[test]
    fn new_file_and_garbage() {
        let t = noted("", "file:///a.jxl", "image/jxl", "'s %u'", NOW).unwrap();
        assert!(t.starts_with("<?xml") && t.trim_end().ends_with("</xbel>"));
        assert_eq!(bookmarks(&t).count(), 1);
        assert_eq!(without_ours(&t).0, format!("{HEADER}</xbel>\n"));
        assert!(noted("{\"json\": true}", "file:///a", "image/png", "x", NOW).is_none());
    }

    #[test]
    fn prunes_only_our_own() {
        let mut t = GTK.to_owned();
        for i in 0..MAX_OURS + 5 {
            t = noted(
                &t,
                &format!("file:///p/{i}.png"),
                "image/png",
                "'s %u'",
                NOW,
            )
            .unwrap();
        }
        let hrefs: Vec<String> = bookmarks(&t).filter_map(|b| b.href()).collect();
        assert_eq!(hrefs.len(), MAX_OURS + 2);
        assert!(hrefs.contains(&"file:///home/me/notes.txt".to_owned()));
        assert!(!hrefs.contains(&"file:///p/0.png".to_owned()));
        assert!(hrefs.contains(&format!("file:///p/{}.png", MAX_OURS + 4)));
    }

    #[test]
    fn option_round_trip() {
        let env = test_env::env("recent");
        let xbel = xbel_path(&env);
        fs::create_dir_all(xbel.parent().unwrap()).unwrap();
        fs::write(&xbel, GTK).unwrap();
        assert_eq!(status(&env).0, State::Off);
        // Off: nothing is written.
        note(&env, Path::new("/p/a.png"), "image/png").unwrap();
        assert_eq!(fs::read_to_string(&xbel).unwrap(), GTK);

        apply(&env).unwrap();
        assert_eq!(status(&env), (State::On, vec!["0".into()]));
        note(&env, Path::new("/p/a.png"), "image/png").unwrap();
        note(&env, Path::new("/p/b.png"), "image/png").unwrap();
        assert_eq!(status(&env), (State::On, vec!["2".into()]));
        let backups = env.record_dir().join("backups");
        assert_eq!(
            fs::read_dir(&backups).unwrap().count(),
            1,
            "backed up before the first change"
        );

        remove(&env).unwrap();
        assert_eq!(fs::read_to_string(&xbel).unwrap(), GTK);
        assert_eq!(status(&env).0, State::Off);
        test_env::cleanup(&env);
    }
}
