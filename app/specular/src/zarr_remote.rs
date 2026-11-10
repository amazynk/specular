//! Zarr stores on servers and in S3 buckets (`specular https://…/image.ome.zarr`,
//! `specular s3://bucket/plate.zarr`, or such an address pasted).
//!
//! Opening an address reads the store's metadata on a worker, lists its images (one image,
//! a plate's wells and fields, a bioformats2raw layout's series), writes one small
//! `.zarrurl` link per image into the cache (one folder per store, so the arrow keys step
//! through a plate) and opens the first. The decoder ([`specular_decode::zarr_url_dec`])
//! opens a link through the store [`install`] registers: chunks are fetched as the view
//! needs them, a level and a region at a time, several requests at once, through the
//! download indicator ("Zarr chunks from <host>").
//!
//! Links hold the address without its query string; a pre-signed address's query and any
//! S3 credentials stay in memory. Credentials come only from what the user configured
//! (`AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY`, or an `AWS_PROFILE` / `default` profile in
//! the shared credentials file), are used only after a bucket refused an unsigned request,
//! and are never written or shown.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use specular_core::keys::{KeyError, KeyStore};
use specular_core::net::What;
use specular_decode::zarr_url_dec::{self, ZarrImage, ZarrLink};
use specular_remote::keystore::display_url;
use specular_remote::s3::{Credentials, ProcessEnv, S3Location};
use specular_remote::{Client, ClientConfig, DiskCache, RemoteStore, StoreOptions};

/// Disk cache for chunks.
const DISK_BUDGET: u64 = 2 << 30;

/// Whether an argument or pasted text names a remote Zarr store: an `s3://` address, or an
/// HTTP(S) address with a path segment ending in `.zarr`.
pub fn is_remote_zarr(arg: &str) -> bool {
    let t = arg.trim();
    if t.contains(char::is_whitespace) {
        return false;
    }
    let l = t.to_ascii_lowercase();
    if let Some(rest) = l.strip_prefix("s3://") {
        return rest.len() >= 3;
    }
    if !(l.starts_with("https://") || l.starts_with("http://")) {
        return false;
    }
    let path = l.split(['?', '#']).next().unwrap_or("");
    path.splitn(4, '/')
        .nth(3)
        .is_some_and(|p| p.split('/').any(|seg| seg.ends_with(".zarr")))
}

#[derive(Default)]
struct Stores {
    /// By the store's root address (no query).
    open: Mutex<HashMap<String, Arc<RemoteStore>>>,
    /// The address as given (with a pre-signed query), by root address; memory only.
    given: Mutex<HashMap<String, String>>,
    cache_dir: Mutex<Option<PathBuf>>,
    disk: OnceLock<Option<Arc<DiskCache>>>,
    client: OnceLock<Option<Client>>,
}

fn stores() -> &'static Stores {
    static S: OnceLock<Stores> = OnceLock::new();
    S.get_or_init(Stores::default)
}

/// The root address a link names: without query, fragment or user, ending in '/'.
pub fn root_of(url: &str) -> String {
    let u = url.trim();
    if u.to_ascii_lowercase().starts_with("s3://") {
        let u = u.split(['?', '#']).next().unwrap_or(u);
        return format!("{}/", u.trim_end_matches('/'));
    }
    format!("{}/", display_url(u).trim_end_matches('/'))
}

/// Registers the `.zarrurl` opener; chunks are cached under `cache_dir` (opened on first
/// use).
pub fn install(cache_dir: PathBuf) {
    *stores().cache_dir.lock() = Some(cache_dir);
    specular_decode::set_zarr_store_opener(Some(Arc::new(|url: &str| {
        store(url).map(|s| s as Arc<dyn KeyStore>)
    })));
}

fn disk() -> Option<Arc<DiskCache>> {
    stores()
        .disk
        .get_or_init(|| {
            let dir = stores().cache_dir.lock().clone()?.join("zarr").join("chunks");
            DiskCache::open(&dir, DISK_BUDGET)
                .inspect_err(|e| {
                    specular_log::warn!(target: specular_log::CACHE, "zarr cache {}: {e}", dir.display())
                })
                .ok()
        })
        .clone()
}

fn client() -> Result<Client, KeyError> {
    stores()
        .client
        .get_or_init(|| {
            Client::new(ClientConfig::default())
                .ok()
                .map(|c| c.tracked(What::ZarrChunks))
        })
        .clone()
        .ok_or_else(|| KeyError::Failed("the network client could not start".into()))
}

/// The store at `url` (one per root address for the process, so its memory cache is kept
/// between images and reopenings).
pub fn store(url: &str) -> Result<Arc<RemoteStore>, KeyError> {
    let root = root_of(url);
    if let Some(s) = stores().open.lock().get(&root) {
        return Ok(s.clone());
    }
    let given = stores()
        .given
        .lock()
        .get(&root)
        .cloned()
        .unwrap_or_else(|| url.to_owned());
    let env = ProcessEnv;
    // Credentials only for a bucket, and only those the user set up.
    let creds = S3Location::parse(&given, &env).and_then(|_| Credentials::configured(&env));
    let s = Arc::new(RemoteStore::open(
        client()?,
        &given,
        &env,
        creds,
        StoreOptions {
            disk: disk(),
            ..StoreOptions::default()
        },
    )?);
    Ok(stores().open.lock().entry(root).or_insert(s).clone())
}

/// Where the links of the store at `url` go.
pub fn links_dir(cache_dir: &Path, url: &str) -> PathBuf {
    let key = DiskCache::url_key(&root_of(url));
    let key = key.rsplit('/').next().unwrap_or("x").to_owned();
    cache_dir.join("zarr").join("links").join(key)
}

/// A file name for an image: its name, else the store's.
fn link_name(url: &str, image: &ZarrImage) -> String {
    let base = if image.name.is_empty() {
        let root = root_of(url);
        let last = root
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("image")
            .to_owned();
        let l = last.to_ascii_lowercase();
        let cut = [".ome.zarr", ".zarr"]
            .iter()
            .find_map(|s| l.strip_suffix(s).map(str::len))
            .unwrap_or(last.len());
        last[..cut].to_owned()
    } else {
        image.name.clone()
    };
    let safe: String = base
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .take(80)
        .collect();
    let safe = safe.trim_matches(['.', ' ']);
    format!("{}.zarrurl", if safe.is_empty() { "image" } else { safe })
}

/// Lists the images of the store at `url` and writes their links into `dir`; the first
/// image's link.
pub fn open_links(url: &str, dir: &Path) -> Result<PathBuf, String> {
    let root = root_of(url);
    stores()
        .given
        .lock()
        .insert(root.clone(), url.trim().to_owned());
    let s = store(url).map_err(|e| crate::error_text::key_reason(&e))?;
    let images =
        zarr_url_dec::images(s.as_ref(), None).map_err(|e| crate::error_text::key_reason(&e))?;
    write_links(dir, &root, &images).map_err(|e| crate::error_text::io_reason(&e))
}

/// Writes one link per image (unchanged files are left alone); the first image's.
pub fn write_links(dir: &Path, root: &str, images: &[ZarrImage]) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let mut first = None;
    let mut used = std::collections::HashSet::new();
    for img in images {
        let mut name = link_name(root, img);
        let mut k = 2;
        while !used.insert(name.clone()) {
            name = format!("{} ({k}).zarrurl", name.trim_end_matches(".zarrurl"));
            k += 1;
        }
        let p = dir.join(&name);
        let bytes = ZarrLink {
            url: root.to_owned(),
            path: img.path.clone(),
            name: img.name.clone(),
        }
        .to_json();
        if std::fs::read(&p).ok().as_deref() != Some(bytes.as_slice()) {
            let mut tmp = p.as_os_str().to_owned();
            tmp.push(".tmp");
            std::fs::write(&tmp, &bytes)?;
            std::fs::rename(&tmp, &p)?;
        }
        first.get_or_insert(p);
    }
    first.ok_or_else(|| std::io::Error::other("no images"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_remote::testing::{ServerOptions, TestServer};
    use specular_remote::zarr_fixture::{self, Spec};

    #[test]
    fn recognises_addresses() {
        assert!(is_remote_zarr("https://h.example/data/6001240.zarr"));
        assert!(is_remote_zarr("https://h.example/a/img.ome.zarr/?sig=1"));
        assert!(is_remote_zarr("http://h/plate.ZARR/A/1"));
        assert!(is_remote_zarr("s3://bucket/x"));
        assert!(!is_remote_zarr("https://h.example.zarr/x.tif"));
        assert!(!is_remote_zarr("/home/me/img.zarr"));
        assert!(!is_remote_zarr("look at https://h/x.zarr"));
        assert_eq!(root_of("https://u:p@h/x.zarr?sig=2"), "https://h/x.zarr/");
        assert_eq!(root_of("s3://b/k.zarr/"), "s3://b/k.zarr/");
    }

    #[test]
    fn link_names_read_well() {
        let one = ZarrImage {
            path: String::new(),
            name: String::new(),
        };
        assert_eq!(
            link_name("https://h/d/6001240.ome.zarr/", &one),
            "6001240.zarrurl"
        );
        let well = ZarrImage {
            path: "A/1/0/".into(),
            name: "A1, field 1".into(),
        };
        assert_eq!(link_name("https://h/p.zarr", &well), "A1, field 1.zarrurl");
    }

    /// A plate opened by address: one link per field, in a folder of their own, each
    /// opening as its image; nothing secret in them.
    #[test]
    fn a_plate_opens_as_a_folder_of_links() {
        let small = Spec {
            size: 64,
            chunk: 32,
            levels: 2,
            planes: 1,
            ..Spec::default()
        };
        let files = Arc::new(zarr_fixture::plate(&small));
        let server = TestServer::start(
            ServerOptions::default(),
            zarr_fixture::handler(files, "/p.zarr/"),
        )
        .unwrap();
        let dir = std::env::temp_dir().join(format!("specular-zarr-links-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        install(dir.join("cache"));
        let url = format!("{}?token=secret", server.url("/p.zarr"));
        let first = open_links(&url, &dir).unwrap();
        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter(|n| n.ends_with(".zarrurl"))
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "A1, field 1.zarrurl",
                "A1, field 2.zarrurl",
                "B2, field 1.zarrurl",
                "B2, field 2.zarrurl"
            ]
        );
        assert_eq!(first.file_name().unwrap(), "A1, field 1.zarrurl");
        // One document per well, the plate's version's, fetched in parallel: no v3 probe
        // for a v2 plate's wells.
        let log = server.log();
        let wells = |doc: &str| {
            log.iter()
                .filter(|r| r.target.contains("/A/1/") || r.target.contains("/B/2/"))
                .filter(|r| r.target.split('?').next().unwrap().ends_with(doc))
                .count()
        };
        assert_eq!((wells("/.zattrs"), wells("/zarr.json")), (2, 0));
        let text = std::fs::read_to_string(dir.join("B2, field 2.zarrurl")).unwrap();
        assert!(!text.contains("secret"), "{text}");
        assert!(
            server
                .log()
                .iter()
                .all(|r| r.param("token").as_deref() == Some("secret"))
        );
        let s = specular_decode::Registry::default()
            .open_path(&dir.join("B2, field 2.zarrurl"))
            .unwrap();
        assert_eq!(s.info().width, 64);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
