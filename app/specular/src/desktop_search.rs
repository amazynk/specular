//! `specular --search-provider`: the desktop search's source (GNOME Shell, KRunner; see
//! `specular_platform::integration::linux::search`), the Library's catalogue.
//!
//! The catalogue is opened read-only ([`specular_library::Snapshot`]) and searched with the
//! Library's query language, as typed in its search bar: newest first, at most a handful of
//! results, cut off after [`BUDGET`]. Pictures only: text files (text, tables, subtitles),
//! offline locations and files that are not readable as images are left out. Thumbnails come
//! from the gallery's cache when it holds one, never made here. Nothing is indexed, written
//! or read from the pictures themselves, and nothing goes on the network.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use specular_library::{Row, Search, Snapshot, Sort};
use specular_platform::integration::linux::search::{Hit, Icon, SearchSource};
use specular_thumbs::{DiskCache, ThumbKey};

/// The longest a query may take.
pub const BUDGET: Duration = Duration::from_millis(300);
/// Thumbnails sent to the desktop are at most this large.
const ICON: u32 = 128;
/// Thumbnail keys of results remembered (several queries' worth).
const KEYS_KEPT: usize = 200;
/// The catalogue's fold (the Library's: `gallery::model::fold`).
const FOLD_ID: &str = "search";

/// The Library's catalogue as a search source.
pub struct LibrarySource {
    catalogue: PathBuf,
    cache: Option<PathBuf>,
    /// Opened on the first query (and again after a failure: the viewer may have made the
    /// catalogue meanwhile).
    snapshot: Mutex<Option<Snapshot>>,
    /// The thumbnail keys of recent results (the desktop asks for icons after a few more
    /// key presses).
    keys: Mutex<HashMap<PathBuf, ThumbKey>>,
}

impl LibrarySource {
    /// The catalogue at `catalogue`, thumbnails from the gallery cache in `cache`.
    pub fn new(catalogue: PathBuf, cache: Option<PathBuf>) -> LibrarySource {
        LibrarySource {
            catalogue,
            cache,
            snapshot: Mutex::new(None),
            keys: Mutex::new(HashMap::new()),
        }
    }

    /// The user's catalogue and thumbnail cache.
    pub fn detect() -> LibrarySource {
        let data = specular_settings::Locations::detect().data_dir;
        LibrarySource::new(
            specular_library::default_path(&data),
            crate::thumbnail::Options::detect().cache,
        )
    }

    fn rows(&self, query: &str, limit: usize) -> Vec<Row> {
        let mut snap = self.snapshot.lock().unwrap_or_else(|e| e.into_inner());
        if snap.is_none() {
            match Snapshot::open(&self.catalogue, specular_i18n::fold_for_search, FOLD_ID) {
                Ok(s) => *snap = Some(s),
                Err(e) => {
                    specular_log::debug!(target: specular_log::SETTINGS, "search: no catalogue: {e}");
                    return Vec::new();
                }
            }
        }
        let Some(s) = snap.as_ref() else {
            return Vec::new();
        };
        let req = Search {
            query: query.to_owned(),
            sort: Sort::Date,
            descending: true,
            ..Search::default()
        };
        let mut rows = Vec::new();
        let r = s.search_each(&req, BUDGET, &mut |row| {
            if rows.len() < limit && is_picture(&row) {
                rows.push(row);
            }
        });
        if let Err(e) = r {
            specular_log::warn!(target: specular_log::SETTINGS, "search: {e}");
            *snap = None;
        }
        rows
    }
}

/// Whether a catalogue row is a picture to offer: not a text file (the rule of folder
/// stepping), readable, and on a location that is online. Decided from the catalogue alone.
pub fn is_picture(row: &Row) -> bool {
    if row.state == 2 || row.offline {
        return false;
    }
    let text = match row.ext.as_str() {
        // Shared with pictures (Bruker SkyScan logs, MSX screens): the facts tell.
        "log" | "scc" => row.meta.as_ref().is_none_or(|m| {
            m.format.is_empty()
                || m.format == specular_decode::Format::PlainText.name()
                || m.format == specular_decode::Format::Scc.name()
        }),
        // By the name only (no read for these extensions).
        _ => specular_decode::text_dec::is_text_file(&row.path),
    };
    !text
}

/// The second line of a result: its folder, with the date taken when known.
fn description(row: &Row) -> String {
    let folder = home_relative(Path::new(&row.folder));
    match row.meta.as_ref().and_then(|m| m.date_taken) {
        Some(secs) => {
            let (y, m, d) = specular_query::date::civil_from_days(secs.div_euclid(86_400));
            format!("{folder} · {y:04}-{m:02}-{d:02}")
        }
        None => folder,
    }
}

/// `~/Pictures/…` for a folder in the home folder.
fn home_relative(p: &Path) -> String {
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from)
        && !home.as_os_str().is_empty()
        && let Ok(rest) = p.strip_prefix(&home)
    {
        return if rest.as_os_str().is_empty() {
            "~".into()
        } else {
            format!("~/{}", rest.display())
        };
    }
    p.display().to_string()
}

impl SearchSource for LibrarySource {
    fn search(&self, query: &str, limit: usize) -> Vec<Hit> {
        let rows = self.rows(query, limit);
        let mut keys = self.keys.lock().unwrap_or_else(|e| e.into_inner());
        if keys.len() > KEYS_KEPT {
            keys.clear();
        }
        rows.into_iter()
            .map(|row| {
                if let Some(k) = row.thumb_key() {
                    keys.insert(row.path.clone(), k);
                }
                Hit {
                    description: description(&row),
                    name: row.name.clone(),
                    path: row.path,
                }
            })
            .collect()
    }

    fn icon(&self, path: &Path) -> Option<Icon> {
        let key = self
            .keys
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(path)
            .copied()?;
        // Budget 0: the cache is only read.
        let cache = DiskCache::new(self.cache.as_deref()?, 0);
        let e = specular_thumbs::CLASSES
            .iter()
            .find_map(|&class| cache.get(&key, class, 0))?;
        let px = e.pixels;
        let (w, h) = specular_thumbs::pixels::fit(px.width, px.height, ICON);
        let rgba = if (w, h) == (px.width, px.height) {
            px.rgba
        } else {
            crate::thumbnail::downscale(
                &px.rgba,
                px.width as usize,
                px.height as usize,
                w as usize,
                h as usize,
            )
        };
        Some(Icon {
            width: w,
            height: h,
            rgba,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_platform::integration::linux::search::{self, Launch, Provider, Texts, client};
    use specular_thumbs::pixels::Pixels;
    use std::sync::Arc;

    fn png(path: &Path, w: u32, h: u32) {
        let rgba: Vec<u8> = (0..w * h)
            .flat_map(|i| [(i % w) as u8, (i / w) as u8, 128, 255])
            .collect();
        std::fs::write(
            path,
            specular_png::encode(w, h, specular_png::ColorType::Rgba, &rgba).unwrap(),
        )
        .unwrap();
    }

    /// A temporary Library of a few pictures and text files, served on a private bus: the
    /// methods GNOME Shell and KRunner call answer from it, pictures only.
    #[test]
    fn library_on_a_private_bus() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/agent-app/test-dsearch")
            .join(std::process::id().to_string());
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("pics/summer trip")).unwrap();
        // Canonical, as the Library stores it. (A folder named after a word would match it:
        // words search the folder path too.)
        let root = root.canonicalize().unwrap();
        let pics = root.join("pics/summer trip");
        png(&pics.join("beach sunset.png"), 300, 120);
        png(&pics.join("beach hut.png"), 40, 30);
        png(&pics.join("forest.png"), 20, 20);
        std::fs::write(pics.join("beach notes.txt"), "a day at the beach\n").unwrap();
        std::fs::write(pics.join("beach list.csv"), "a,b\n1,2\n").unwrap();
        std::fs::write(pics.join("beach run.log"), "started\n").unwrap();

        let catalogue = root.join("library.sqlite3");
        // Before the Library exists: nothing, and nothing is made.
        let source = LibrarySource::new(catalogue.clone(), Some(root.join("cache")));
        assert!(source.search("beach", 10).is_empty());
        assert!(!catalogue.exists());

        let mut cfg = specular_library::Config::new(catalogue.clone());
        cfg.fold = specular_i18n::fold_for_search;
        cfg.fold_id = FOLD_ID;
        cfg.watch = false;
        let lib = specular_library::Library::open(cfg).unwrap();
        lib.add_location(&root.join("pics"));
        assert!(
            lib.wait_idle(Duration::from_secs(30)),
            "indexer did not settle"
        );
        let all = lib
            .search(&Search {
                query: "beach".into(),
                ..Search::default()
            })
            .unwrap();
        assert!(
            all.rows.iter().any(|r| r.name.ends_with(".txt")),
            "the catalogue holds the text file: the provider leaves it out"
        );

        // A gallery-cache thumbnail for one of them.
        let sunset = pics.join("beach sunset.png");
        let cache = DiskCache::new(root.join("cache"), 64 << 20);
        let key = ThumbKey::of_file(&sunset).unwrap();
        let tile = Pixels::new(256, 102, [9u8, 200, 30, 255].repeat(256 * 102));
        let meta = specular_thumbs::Meta {
            width: 300,
            height: 120,
            format: "PNG".into(),
            ..Default::default()
        };
        cache.put(&key, 256, 0, &tile, &meta).unwrap();

        let Some(bus) = specular_platform::test_bus::Bus::start("dsearch") else {
            eprintln!("no dbus-daemon: skipped");
            let _ = std::fs::remove_dir_all(&root);
            return;
        };
        let launched = Arc::new(Mutex::new(Vec::new()));
        let l = launched.clone();
        let provider = Provider {
            source: Box::new(source),
            texts: Texts {
                open_folder: "Open folder".into(),
            },
            launch: Box::new(move |x| l.lock().unwrap().push(x)),
        };
        let address = bus.address.clone();
        let server = specular_thread::spawn("dsearch-test", move || {
            search::serve_on(Some(&address), provider, Duration::from_secs(2))
        });
        let a = bus.address.as_str();
        client::wait_for(a);

        let ids = client::initial(a, &["beach"]);
        let mut sorted = ids.clone();
        sorted.sort();
        let hut = pics.join("beach hut.png").to_string_lossy().into_owned();
        let sunset_id = sunset.to_string_lossy().into_owned();
        assert_eq!(sorted, [hut.clone(), sunset_id.clone()], "pictures only");
        // The query language, as in the Library's search bar.
        assert_eq!(
            client::subsearch(a, &ids, &["beach", "sunset"]),
            std::slice::from_ref(&sunset_id)
        );
        assert_eq!(
            client::initial(a, &["beach", "-sunset"]),
            std::slice::from_ref(&hut)
        );
        assert_eq!(client::initial(a, &["forest", "ext:png"]).len(), 1);
        assert!(client::initial(a, &["notes"]).is_empty(), "no text files");
        assert!(client::initial(a, &["run"]).is_empty());

        let metas = client::metas(a, &[sunset_id.clone(), hut.clone()]);
        assert_eq!(metas[0].name, "beach sunset.png");
        assert!(
            metas[0].description.ends_with("summer trip"),
            "{}",
            metas[0].description
        );
        assert_eq!(
            metas[0].icon,
            Some((128, 51)),
            "from the gallery cache, made smaller"
        );
        assert_eq!(
            metas[1].icon, None,
            "not in the cache: no thumbnail is made"
        );
        assert_eq!(metas[1].gicon.as_deref(), Some("image-x-generic"));

        let m = client::krunner_match(a, "beach sunset");
        assert_eq!(m.len(), 1);
        assert!(m[0].3);
        client::activate(a, &sunset_id);
        client::krunner_run(a, &hut, "folder");
        client::launch_search(a, &["tag:sea"]);
        assert_eq!(
            *launched.lock().unwrap(),
            [
                Launch::Open(sunset.clone()),
                Launch::Folder(pics.clone()),
                Launch::Library("tag:sea".into()),
            ]
        );
        assert!(matches!(server.join().unwrap(), Ok(true)));
        drop(lib);
        let _ = std::fs::remove_dir_all(&root);
    }

    fn row(path: &str, state: u8, format: Option<&str>) -> Row {
        let p = PathBuf::from(path);
        Row {
            id: 1,
            name: p.file_name().unwrap().to_string_lossy().into_owned(),
            folder: p.parent().unwrap().to_string_lossy().into_owned(),
            ext: p.extension().unwrap().to_string_lossy().to_lowercase(),
            path: p,
            size: 1,
            mtime_ns: 0,
            state,
            meta: format.map(|f| specular_thumbs::Meta {
                format: f.into(),
                ..Default::default()
            }),
            mark: Default::default(),
            tags: String::new(),
            content: None,
            classes: 0,
            offline: false,
            dupes: None,
            similar: None,
            place: None,
            people: None,
            words: None,
            develop: None,
            albums: None,
            iptc: None,
        }
    }

    #[test]
    fn pictures_only() {
        assert!(is_picture(&row("/p/a.jpg", 1, Some("JPEG"))));
        assert!(is_picture(&row("/p/a.cr3", 0, None)), "facts not read yet");
        assert!(!is_picture(&row("/p/a.jpg", 2, None)), "not readable");
        for t in [
            "/p/a.txt",
            "/p/a.csv",
            "/p/a.md",
            "/p/a.srt",
            "/p/a.rtf",
            "/p/a.ipynb",
        ] {
            assert!(!is_picture(&row(t, 1, Some("Plain text"))), "{t}");
        }
        assert!(!is_picture(&row("/p/a.log", 0, None)));
        assert!(!is_picture(&row("/p/a.log", 1, Some("Plain text"))));
        assert!(is_picture(&row("/p/a.log", 1, Some("Bruker SkyScan"))));
        assert!(!is_picture(&row("/p/a.scc", 1, Some("Scenarist captions"))));
        assert!(is_picture(&row("/p/a.scc", 1, Some("MSX screen"))));
        let mut off = row("/p/a.jpg", 1, None);
        off.offline = true;
        assert!(!is_picture(&off));
    }

    #[test]
    fn descriptions() {
        let mut r = row("/srv/p/a.jpg", 1, Some("JPEG"));
        assert_eq!(description(&r), "/srv/p");
        if let Some(m) = r.meta.as_mut() {
            m.date_taken =
                Some(specular_query::date::days_from_civil(2024, 7, 1) * 86_400 + 36_000);
        }
        assert_eq!(description(&r), "/srv/p · 2024-07-01");
    }
}
