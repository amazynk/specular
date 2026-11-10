//! 3D Tiles on a server (`specular https://…/tileset.json`): the tileset's JSON is fetched
//! into the cache with its address beside it, then opens like a local tileset
//! (`specular_decode`'s 3D Tiles reader). The reader's fetcher ([`install`]) brings external
//! tilesets, subtrees and tile contents as the view needs them: through the tracked client
//! (the downloads indicator shows the host; the no-network switch stops them), from the
//! tileset's own server only, kept in the cache.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_remote::{CancelToken, Client, ClientConfig, DiskCache, Request};

/// The cache's budget for tile files.
const DISK_BUDGET: u64 = 2 << 30;

/// Whether an address names a tileset by its file name (`tileset.json`,
/// `tileset_1.1.json`).
pub fn is_remote_tileset(arg: &str) -> bool {
    let l = arg.trim().to_ascii_lowercase();
    if !(l.starts_with("https://") || l.starts_with("http://")) {
        return false;
    }
    let path = l.split(['?', '#']).next().unwrap_or("");
    let name = path.rsplit('/').next().unwrap_or("");
    name.starts_with("tileset") && name.ends_with(".json")
}

fn client(cfg: &ClientConfig) -> Result<Client, String> {
    Ok(Client::new(cfg.clone())
        .map_err(|e| crate::error_text::reason(&e))?
        .tracked(specular_core::net::What::Data))
}

fn get(c: &Client, url: &str, max: u64) -> Result<Vec<u8>, String> {
    let r = c
        .get(&Request::get(url).limit(max), &CancelToken::new())
        .map_err(|e| crate::error_text::reason(&e))?;
    match r.status {
        200 => Ok(r.body),
        s => Err(format!("the server answered {s}")),
    }
}

/// Writes the tileset `json` fetched from `url` into `dir` with its address; returns the
/// local file to open.
pub fn store(url: &str, json: &[u8], dir: &Path) -> Result<PathBuf, String> {
    let key = DiskCache::url_key(url);
    let key = key.rsplit('/').next().unwrap_or("x").to_owned();
    let dir = dir.join(key);
    std::fs::create_dir_all(&dir).map_err(|e| crate::error_text::io_reason(&e))?;
    let path = dir.join("tileset.json");
    std::fs::write(&path, json).map_err(|e| crate::error_text::io_reason(&e))?;
    std::fs::write(dir.join(specular_decode::TILES3D_ADDRESS_FILE), url)
        .map_err(|e| crate::error_text::io_reason(&e))?;
    Ok(path)
}

fn fetch_to(cfg: &ClientConfig, url: &str, dir: &Path) -> Result<PathBuf, String> {
    let json = get(
        &client(cfg)?,
        url,
        specular_decode::specular_tiles3d::MAX_JSON as u64,
    )?;
    if !specular_decode::specular_tiles3d::sniff(&json) {
        return Err("the address is not a 3D Tiles tileset".into());
    }
    store(url, &json, dir)
}

/// The folder remote tilesets are kept in.
pub fn tilesets_dir() -> PathBuf {
    specular_settings::Locations::detect()
        .cache_dir
        .join("3dtiles")
}

/// For an address given on the command line or in the address bar.
pub fn fetch_arg(url: &str) -> Result<PathBuf, String> {
    fetch_to(&ClientConfig::default(), url, &tilesets_dir())
}

/// Reads a remote tileset's files, keeping them in the cache.
struct Http {
    cfg: ClientConfig,
    cache: Option<Arc<DiskCache>>,
}

impl specular_decode::specular_tiles3d::Fetch for Http {
    fn get(&self, key: &str, max: usize) -> Result<Vec<u8>, String> {
        let rel = DiskCache::url_key(key);
        if let Some(p) = self.cache.as_ref().and_then(|c| c.get(&rel))
            && let Ok(b) = std::fs::read(p)
        {
            return Ok(b);
        }
        let b = get(&client(&self.cfg)?, key, max as u64)?;
        if let Some(c) = &self.cache {
            let _ = c.insert_bytes(&rel, &b);
        }
        Ok(b)
    }
}

/// Registers the 3D Tiles reader's fetcher; tile files are kept under `cache_dir`.
pub fn install(cache_dir: PathBuf) {
    install_with(ClientConfig::default(), cache_dir);
}

fn install_with(cfg: ClientConfig, cache_dir: PathBuf) {
    let cache = DiskCache::open(cache_dir.join("3dtiles-files"), DISK_BUDGET).ok();
    specular_decode::set_tiles3d_remote(Some(Arc::new(Http { cfg, cache })));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reader's fetcher is one for the process: the tests that set it take turns.
    static FETCHER: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn tileset_addresses() {
        assert!(is_remote_tileset("https://h/city/tileset.json"));
        assert!(is_remote_tileset("http://h/a/tileset_1.1.json?v=2"));
        assert!(!is_remote_tileset("https://h/item.json"));
        assert!(!is_remote_tileset("/local/tileset.json"));
    }

    /// A tileset on a local test server: the root is cached with its address, the tile it
    /// names comes through the fetcher, and a reference to another server is refused.
    #[test]
    fn opens_a_tileset_over_http() {
        let _turn = FETCHER.lock().unwrap_or_else(|e| e.into_inner());
        use specular_remote::testing::{ServerOptions, TestResponse, TestServer};
        let glb = specular_decode::specular_mesh::fixtures::glb();
        let mut b3dm = Vec::new();
        let ft = b"{\"BATCH_LENGTH\":0}      ";
        let total = 28 + ft.len() + glb.len();
        b3dm.extend_from_slice(b"b3dm");
        for v in [1u32, total as u32, ft.len() as u32, 0, 0, 0] {
            b3dm.extend_from_slice(&v.to_le_bytes());
        }
        b3dm.extend_from_slice(ft);
        b3dm.extend_from_slice(&glb);
        let ts = br#"{"asset":{"version":"1.0"},"geometricError":10,"root":{"boundingVolume":{"sphere":[0,0,0,2]},
            "geometricError":0,"content":{"uri":"t/a.b3dm"},
            "children":[{"boundingVolume":{"sphere":[0,0,0,2]},"geometricError":0,"content":{"uri":"https://elsewhere.example/x.b3dm"}}]}}"#
            .to_vec();
        let server = TestServer::start(ServerOptions::default(), move |r| match r.path() {
            "/set/tileset.json" => TestResponse::ok(ts.clone()),
            "/set/t/a.b3dm" => TestResponse::ok(b3dm.clone()),
            _ => TestResponse::status(404),
        })
        .unwrap();
        let dir =
            std::env::temp_dir().join(format!("specular-3dtiles-http-{}", std::process::id()));
        install_with(ClientConfig::for_tests(), dir.clone());
        let url = server.url("/set/tileset.json");
        let local = fetch_to(&ClientConfig::for_tests(), &url, &dir).unwrap();
        let s = specular_decode::Registry::default()
            .open_path(&local)
            .unwrap();
        assert_eq!(s.info().format, specular_decode::Format::Tiles3d);
        assert!(s.mesh().is_some());
        let h = s.header().unwrap();
        assert!(
            h.fields
                .iter()
                .any(|(k, v)| k == "not read" && v.contains("outside the tileset")),
            "{:?}",
            h.fields
        );
        // Cached: a second fetch of the tile needs no server.
        drop(server);
        let again = specular_decode::Registry::default().open_path(&local);
        assert!(again.is_ok());
        specular_decode::set_tiles3d_remote(None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A signed tileset: the server answers only requests carrying the key, and the key
    /// of the address opened goes with the child tileset, its tile and the glTF's buffer.
    #[test]
    fn signed_addresses_carry_their_key_to_every_file() {
        let _turn = FETCHER.lock().unwrap_or_else(|e| e.into_inner());
        use specular_remote::testing::{ServerOptions, TestResponse, TestServer};
        use std::sync::Mutex;
        let (gltf, bin) = specular_decode::specular_mesh::fixtures::gltf_side();
        let root = br#"{"asset":{"version":"1.0"},"geometricError":10,"root":{"boundingVolume":{"sphere":[0,0,0,4]},
            "geometricError":10,"refine":"REPLACE","children":[{"boundingVolume":{"sphere":[0,0,0,4]},"geometricError":5,
            "content":{"uri":"sub/tileset.json"}}]}}"#
            .to_vec();
        let sub = br#"{"asset":{"version":"1.0"},"geometricError":5,"root":{"boundingVolume":{"sphere":[0,0,0,4]},
            "geometricError":0,"content":{"uri":"m/cube.gltf"}}}"#
            .to_vec();
        let asked = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen = asked.clone();
        let server = TestServer::start(ServerOptions::default(), move |r| {
            let signed = r.query().iter().any(|(k, v)| k == "sig" && v == "s3cret");
            seen.lock().unwrap().push(format!("{} {signed}", r.path()));
            if !signed {
                return TestResponse::status(403);
            }
            match r.path() {
                "/set/tileset.json" => TestResponse::ok(root.clone()),
                "/set/sub/tileset.json" => TestResponse::ok(sub.clone()),
                "/set/sub/m/cube.gltf" => TestResponse::ok(gltf.clone()),
                "/set/sub/m/cube.bin" => TestResponse::ok(bin.clone()),
                _ => TestResponse::status(404),
            }
        })
        .unwrap();
        let dir =
            std::env::temp_dir().join(format!("specular-3dtiles-signed-{}", std::process::id()));
        install_with(ClientConfig::for_tests(), dir.clone());
        let url = server.url("/set/tileset.json?sig=s3cret");
        let local = fetch_to(&ClientConfig::for_tests(), &url, &dir).unwrap();
        let s = specular_decode::Registry::default()
            .open_path(&local)
            .unwrap();
        let m = s.mesh().expect("the child tileset's model was read");
        assert!(m.mesh.triangle_count() > 0);
        let asked = asked.lock().unwrap().clone();
        for p in [
            "/set/tileset.json",
            "/set/sub/tileset.json",
            "/set/sub/m/cube.gltf",
            "/set/sub/m/cube.bin",
        ] {
            assert!(asked.contains(&format!("{p} true")), "{p}: {asked:?}");
        }
        assert!(asked.iter().all(|a| a.ends_with("true")), "{asked:?}");
        specular_decode::set_tiles3d_remote(None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
