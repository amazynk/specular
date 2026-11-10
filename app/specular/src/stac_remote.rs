//! STAC on a server: an item, item collection, collection or catalog address
//! (`https://…/item.json`) is fetched into the cache, with its address recorded as its
//! `self` link so relative links still resolve, then opens like a local STAC file
//! (`specular_decode`'s STAC reader). The reader's fetcher ([`install`]) brings linked
//! documents and GeoTIFF assets the same way: through the tracked client (the downloads
//! indicator shows them; the no-network switch stops them), COGs as their fitting level.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_remote::{CancelToken, Client, ClientConfig, Request};

/// A STAC document's bytes at most.
const MAX_JSON: u64 = 16 << 20;

/// Whether an address names a STAC document: a `.json` path, or a STAC API item path
/// (`/collections/<id>/items/<id>`).
pub fn is_remote_stac(arg: &str) -> bool {
    let l = arg.trim().to_ascii_lowercase();
    if !(l.starts_with("https://") || l.starts_with("http://")) {
        return false;
    }
    let path = l.split(['?', '#']).next().unwrap_or("");
    path.ends_with(".json") || path.contains("/collections/") && path.contains("/items/")
}

fn client(cfg: &ClientConfig, what: specular_core::net::What) -> Result<Client, String> {
    Ok(Client::new(cfg.clone())
        .map_err(|e| crate::error_text::reason(&e))?
        .tracked(what))
}

/// The bytes of a JSON document on a server.
fn fetch_json(cfg: &ClientConfig, url: &str) -> Result<Vec<u8>, String> {
    let c = client(cfg, specular_core::net::What::MapService)?;
    let r = c
        .get(&Request::get(url).limit(MAX_JSON), &CancelToken::new())
        .map_err(|e| crate::error_text::reason(&e))?;
    match r.status {
        200 => Ok(r.body),
        s => Err(format!("the server answered {s}")),
    }
}

/// `json` with a `self` link to `url` when it has none.
fn with_self_link(json: &[u8], url: &str) -> Vec<u8> {
    let Ok(mut v) = serde_json::from_slice::<serde_json::Value>(json) else {
        return json.to_vec();
    };
    let Some(o) = v.as_object_mut() else {
        return json.to_vec();
    };
    let links = o
        .entry("links")
        .or_insert_with(|| serde_json::Value::Array(Vec::new()));
    if let Some(a) = links.as_array_mut()
        && !a
            .iter()
            .any(|l| l.get("rel").and_then(|r| r.as_str()) == Some("self"))
    {
        a.push(serde_json::json!({ "rel": "self", "href": url }));
    }
    serde_json::to_vec_pretty(&v).unwrap_or_else(|_| json.to_vec())
}

/// Fetches the document at `url` into `dir` and returns the local copy to open.
fn fetch_to(cfg: &ClientConfig, url: &str, dir: &Path) -> Result<PathBuf, String> {
    let json = fetch_json(cfg, url)?;
    // A `.json` address may be a 3D Tiles tileset.
    if specular_decode::specular_tiles3d::sniff(&json) {
        return crate::tiles3d_remote::store(url, &json, &crate::tiles3d_remote::tilesets_dir());
    }
    if !specular_decode::geo::specular_gis::stac::sniff(&json) {
        return Err("the address is not a STAC document".into());
    }
    let key = specular_remote::DiskCache::url_key(url);
    let key = key.rsplit('/').next().unwrap_or("x").to_owned();
    let dir = dir.join(key);
    std::fs::create_dir_all(&dir).map_err(|e| crate::error_text::io_reason(&e))?;
    let path = dir.join("stac.json");
    std::fs::write(&path, with_self_link(&json, url))
        .map_err(|e| crate::error_text::io_reason(&e))?;
    Ok(path)
}

/// For an address given on the command line or in the address bar.
pub fn fetch_arg(url: &str) -> Result<PathBuf, String> {
    if crate::tiles3d_remote::is_remote_tileset(url) {
        return crate::tiles3d_remote::fetch_arg(url);
    }
    let dir = specular_settings::Locations::detect()
        .cache_dir
        .join("stac");
    fetch_to(&ClientConfig::default(), url, &dir)
}

/// Registers the STAC reader's fetcher; COG levels are kept under `cache_dir`.
pub fn install(cache_dir: PathBuf) {
    install_with(ClientConfig::default(), cache_dir);
}

fn install_with(cfg: ClientConfig, cache_dir: PathBuf) {
    let c2 = cfg.clone();
    specular_decode::set_stac_remote(Some(Arc::new(specular_decode::StacRemote {
        json: Box::new(move |url| fetch_json(&cfg, url)),
        geotiff: Box::new(move |url| {
            let client = client(&c2, specular_core::net::What::ImageTiles)?;
            crate::cog_remote::fetch_to(
                &client,
                url,
                &cache_dir.join("cog"),
                &CancelToken::new(),
                &mut |_| {},
            )
            .map(|(p, _)| p)
        }),
    })));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stac_addresses() {
        assert!(is_remote_stac("https://h/catalog/items/S2.json"));
        assert!(is_remote_stac(
            "https://earth-search.aws.element84.com/v1/collections/sentinel-2-l2a/items/S2B_33TUN"
        ));
        assert!(!is_remote_stac("https://h/x.tif"));
        assert!(!is_remote_stac("/local/item.json"));
        let j = with_self_link(
            br#"{"type":"Feature","stac_version":"1.0.0"}"#,
            "https://h/i.json",
        );
        let v: serde_json::Value = serde_json::from_slice(&j).unwrap();
        assert_eq!(v["links"][0]["href"], "https://h/i.json");
        // An existing self link stays.
        let j = with_self_link(
            br#"{"links":[{"rel":"self","href":"https://a/b.json"}]}"#,
            "https://h/i.json",
        );
        let v: serde_json::Value = serde_json::from_slice(&j).unwrap();
        assert_eq!(v["links"].as_array().unwrap().len(), 1);
    }

    /// An item on a local test server whose asset is a COG beside it: the item is cached
    /// with its address, and the asset fetched through the fetcher opens georeferenced.
    #[test]
    fn opens_an_item_and_its_cog_over_http() {
        use specular_decode::geo::specular_gis::fixtures::{self, GeoGrid};
        use specular_remote::testing::{ServerOptions, TestResponse, TestServer};
        let grid = GeoGrid {
            epsg: 32633,
            origin: (500_000.0, 4_200_000.0),
            pixel: (10.0, 10.0),
        };
        let values = fixtures::dem_values(300, 200, false);
        let cog = fixtures::geotiff_f32(&values, 300, 200, grid, None, 64);
        let item = br#"{"type":"Feature","stac_version":"1.0.0","id":"it","properties":{},
            "assets":{"dem":{"href":"dem.tif","type":"image/tiff; application=geotiff; profile=cloud-optimized","roles":["data"]}}}"#
            .to_vec();
        let server = TestServer::start(ServerOptions::default(), move |r| match r.path() {
            "/stac/item.json" => TestResponse::ok(item.clone()),
            "/stac/dem.tif" => TestResponse::ranged(r, &cog, "\"v1\""),
            _ => TestResponse::status(404),
        })
        .unwrap();
        let dir = std::env::temp_dir().join(format!("specular-stac-http-{}", std::process::id()));
        install_with(ClientConfig::for_tests(), dir.clone());
        let url = server.url("/stac/item.json");
        let local = fetch_to(&ClientConfig::for_tests(), &url, &dir).unwrap();
        let s = specular_decode::Registry::default()
            .open_path(&local)
            .unwrap();
        assert_eq!((s.info().width, s.info().height), (300, 200));
        assert!(s.geo().is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
