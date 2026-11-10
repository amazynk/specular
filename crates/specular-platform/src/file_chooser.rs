//! The desktop's Open dialog through the `org.freedesktop.portal.FileChooser` portal (Linux
//! and BSD), with a chosen filter selected when it opens (`current_filter`). The dialog
//! crate the app uses elsewhere sends no `current_filter`, and KDE's dialog then does not
//! start on the first filter. Windows and macOS keep the app's own dialog code.
//!
//! The call blocks until the user answers: run it on a background thread.

use std::path::{Path, PathBuf};

/// One entry of the dialog's file type list: a name and its glob patterns (`*.jpg`, `*`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileFilter {
    pub name: String,
    pub patterns: Vec<String>,
}

impl FileFilter {
    /// A filter for `extensions` (no dot), each in lower and upper case: some dialogs (the
    /// portal's on KDE and GNOME) match patterns case-sensitively.
    pub fn from_extensions<'a>(name: &str, extensions: impl IntoIterator<Item = &'a str>) -> Self {
        let mut patterns = Vec::new();
        for e in extensions {
            patterns.push(format!("*.{e}"));
            let up = e.to_ascii_uppercase();
            if up != e {
                patterns.push(format!("*.{up}"));
            }
        }
        FileFilter {
            name: name.to_owned(),
            patterns,
        }
    }

    /// The "All files" entry.
    pub fn any(name: &str) -> Self {
        FileFilter {
            name: name.to_owned(),
            patterns: vec!["*".to_owned()],
        }
    }
}

/// What to ask the portal for.
#[derive(Clone, Debug)]
pub struct OpenFile<'a> {
    pub title: &'a str,
    pub filters: &'a [FileFilter],
    /// Index into `filters` of the one selected when the dialog opens.
    pub current: usize,
    pub folder: Option<&'a Path>,
}

/// Whether the portal is likely there (its D-Bus service is installed and a bus runs).
pub fn portal_available() -> bool {
    #[cfg(all(unix, not(target_os = "macos")))]
    return crate::share::share_available();
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    return false;
}

/// Shows the portal's Open dialog: `Ok(None)` when the user cancels, an error when there is
/// no portal or it fails (the caller falls back to another dialog).
pub fn open_file(req: &OpenFile<'_>) -> std::io::Result<Option<PathBuf>> {
    #[cfg(all(unix, not(target_os = "macos")))]
    return portal::open_file(req);
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    {
        let _ = req;
        Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod portal {
    use std::collections::HashMap;
    use std::io;
    use std::os::unix::ffi::OsStrExt;
    use std::path::PathBuf;

    use zbus::zvariant::{OwnedValue, Value};

    use super::{FileFilter, OpenFile};

    const PORTAL_BUS: &str = "org.freedesktop.portal.Desktop";
    const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
    const IFACE: &str = "org.freedesktop.portal.FileChooser";

    fn bus_err(e: impl std::fmt::Display) -> io::Error {
        io::Error::other(format!("D-Bus: {e}"))
    }

    /// A filter as the portal takes it: `(sa(us))`, every pattern a glob (type 0).
    fn wire(f: &FileFilter) -> (String, Vec<(u32, String)>) {
        (
            f.name.clone(),
            f.patterns.iter().map(|p| (0u32, p.clone())).collect(),
        )
    }

    /// The `OpenFile` options (`a{sv}`) for `req`.
    pub(super) fn options<'v>(
        req: &OpenFile<'_>,
        token: &'v str,
    ) -> HashMap<&'static str, Value<'v>> {
        let mut o: HashMap<&'static str, Value<'v>> = HashMap::new();
        o.insert("handle_token", Value::from(token));
        o.insert("multiple", Value::from(false));
        o.insert("modal", Value::from(true));
        let filters: Vec<_> = req.filters.iter().map(wire).collect();
        if !filters.is_empty() {
            o.insert("filters", Value::new(filters));
        }
        if let Some(f) = req.filters.get(req.current) {
            o.insert("current_filter", Value::new(wire(f)));
        }
        if let Some(dir) = req.folder {
            // A NUL-terminated byte string.
            let mut b = dir.as_os_str().as_bytes().to_vec();
            b.push(0);
            o.insert("current_folder", Value::new(b));
        }
        o
    }

    pub(super) fn open_file(req: &OpenFile<'_>) -> io::Result<Option<PathBuf>> {
        if !super::portal_available() {
            return Err(io::Error::from(io::ErrorKind::NotFound));
        }
        let conn = zbus::blocking::Connection::session().map_err(bus_err)?;
        let proxy = zbus::blocking::proxy::Builder::<zbus::blocking::Proxy>::new(&conn)
            .destination(PORTAL_BUS)
            .and_then(|b| b.path(PORTAL_PATH))
            .and_then(|b| b.interface(IFACE))
            .map(|b| b.cache_properties(zbus::proxy::CacheProperties::No))
            .and_then(|b| b.build())
            .map_err(bus_err)?;
        let token = format!("specularopen{}", std::process::id());
        let sender = conn
            .unique_name()
            .map(|n| n.trim_start_matches(':').replace('.', "_"))
            .ok_or_else(|| bus_err("no unique name"))?;
        let request_path = format!("{PORTAL_PATH}/request/{sender}/{token}");
        // Subscribe before calling, so the answer cannot be missed.
        let request = zbus::blocking::Proxy::new(
            &conn,
            PORTAL_BUS,
            request_path.as_str(),
            "org.freedesktop.portal.Request",
        )
        .map_err(bus_err)?;
        let mut responses = request.receive_signal("Response").map_err(bus_err)?;
        proxy
            .call_method("OpenFile", &("", req.title, options(req, &token)))
            .map_err(bus_err)?;
        // The user may keep the dialog open as long as they like.
        let msg = responses
            .next()
            .ok_or_else(|| bus_err("no answer from the portal"))?;
        let (code, results) = msg
            .body()
            .deserialize::<(u32, HashMap<String, OwnedValue>)>()
            .map_err(bus_err)?;
        match code {
            0 => Ok(first_path(&results)),
            // 1: cancelled.
            1 => Ok(None),
            c => Err(bus_err(format!("the portal answered {c}"))),
        }
    }

    fn first_path(results: &HashMap<String, OwnedValue>) -> Option<PathBuf> {
        let uris = results.get("uris")?.try_clone().ok()?;
        let uris = Vec::<String>::try_from(uris).ok()?;
        uris.iter().find_map(|u| crate::launch::path_from_uri(u))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_take_both_cases() {
        let f = FileFilter::from_extensions("Pictures", ["jpg", "mp+", "123"]);
        assert_eq!(f.patterns, ["*.jpg", "*.JPG", "*.mp+", "*.MP+", "*.123"]);
        assert_eq!(FileFilter::any("All").patterns, ["*"]);
    }

    /// The options name the first filter as the current one, with the very same contents
    /// (KDE selects it by equality), and the folder as a NUL-terminated byte string. Nothing
    /// is sent: no dialog opens.
    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn options_select_the_current_filter() {
        use zbus::zvariant::Value;
        let filters = [
            FileFilter::from_extensions("All supported files", ["jpg", "png"]),
            FileFilter::from_extensions("Images", ["jpg"]),
            FileFilter::any("All files"),
        ];
        let req = OpenFile {
            title: "Open",
            filters: &filters,
            current: 0,
            folder: Some(Path::new("/pics")),
        };
        let o = portal::options(&req, "tok");
        assert_eq!(o["handle_token"], Value::from("tok"));
        assert_eq!(o["multiple"], Value::from(false));
        let list = &o["filters"];
        assert_eq!(list.value_signature().to_string(), "a(sa(us))");
        let cur = &o["current_filter"];
        assert_eq!(cur.value_signature().to_string(), "(sa(us))");
        let Value::Array(a) = list else {
            panic!("filters: {list:?}")
        };
        assert_eq!(a.len(), 3);
        assert_eq!(&a[0], cur);
        assert_eq!(o["current_folder"], Value::new(b"/pics\0".to_vec()));
    }
}
