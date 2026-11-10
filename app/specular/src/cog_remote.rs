//! A Cloud-Optimized GeoTIFF on an HTTP server (`specular https://…/image.tif`): its header
//! and directories are read by range requests, then the tiles of the finest level that fits
//! the budgets, kept as a smaller GeoTIFF in the cache, which then opens like a local one.

use std::path::PathBuf;

use specular_decode::geo::specular_gis::cog::{self, Fetched};
use specular_decode::geo::specular_gis::pmtiles::RangeRead;
use specular_remote::{CancelToken, Client, ClientConfig, Request};

/// Pixels fetched at most (the finest level under this; the coarsest otherwise).
pub const MAX_PIXELS: u64 = 64_000_000;
/// Compressed tile bytes fetched at most.
pub const MAX_BYTES: u64 = 512 << 20;

/// Whether a command-line argument names a remote GeoTIFF.
pub fn is_remote_cog(arg: &str) -> bool {
    let l = arg.trim().to_ascii_lowercase();
    (l.starts_with("https://") || l.starts_with("http://"))
        && l.split(['?', '#'])
            .next()
            .is_some_and(|p| p.ends_with(".tif") || p.ends_with(".tiff"))
}

struct Http<'a> {
    client: &'a Client,
    url: &'a str,
    cancel: &'a CancelToken,
}

impl RangeRead for Http<'_> {
    fn read_range(&self, offset: u64, len: u64) -> Result<Vec<u8>, String> {
        let req = Request::get(self.url)
            .range(offset..offset + len)
            .limit(len + 1);
        let r = self
            .client
            .get(&req, self.cancel)
            .map_err(|e| crate::error_text::reason(&e))?;
        match r.status {
            // The last range of a file may come back shorter than asked.
            206 => Ok(r.body),
            200 => Err("the server does not answer range requests".into()),
            s => Err(format!("the server answered {s}")),
        }
    }
}

/// The level fetched from `url`, as a GeoTIFF in `dir`; `progress(bytes)`.
pub fn fetch_to(
    client: &Client,
    url: &str,
    dir: &std::path::Path,
    cancel: &CancelToken,
    progress: &mut dyn FnMut(u64),
) -> Result<(PathBuf, Fetched), String> {
    let src = Http {
        client,
        url,
        cancel,
    };
    let mut f = cog::fetch_level(&src, MAX_PIXELS, MAX_BYTES, progress)?;
    let name = url
        .split(['?', '#'])
        .next()
        .and_then(|p| p.rsplit('/').next())
        .filter(|n| !n.is_empty())
        .unwrap_or("image.tif");
    let key = specular_remote::DiskCache::url_key(url);
    let key = key.rsplit('/').next().unwrap_or("x").to_owned();
    let dir = dir.join(key);
    std::fs::create_dir_all(&dir).map_err(|e| crate::error_text::io_reason(&e))?;
    let path = dir.join(specular_remote::cache::sanitize(name));
    std::fs::write(&path, std::mem::take(&mut f.image))
        .map_err(|e| crate::error_text::io_reason(&e))?;
    Ok((path, f))
}

/// For a GeoTIFF address given on the command line: fetches it into the cache (on the
/// window's worker, so "Downloads" shows it) and returns the local file to open, or why not.
pub fn fetch_arg(url: &str) -> Result<PathBuf, String> {
    let client = Client::new(ClientConfig::default()).map_err(|e| e.to_string())?;
    let dir = specular_settings::Locations::detect().cache_dir.join("cog");
    fetch_arg_with(&client, url, &dir)
}

/// [`fetch_arg`] with the client and cache folder given: every range request is counted
/// in one "Image tiles from <host>" row of the client's download registry.
pub fn fetch_arg_with(
    client: &Client,
    url: &str,
    dir: &std::path::Path,
) -> Result<PathBuf, String> {
    let client = client.tracked(specular_core::net::What::ImageTiles);
    let cancel = CancelToken::new();
    let t0 = std::time::Instant::now();
    let (p, f) = fetch_to(&client, url, dir, &cancel, &mut |_| {})?;
    specular_log::info!(
        target: specular_log::IO,
        "cog: level {} of {} ({}×{} of {}×{}), {} bytes in {} requests, {:.1} s",
        f.level,
        f.levels,
        f.width,
        f.height,
        f.full.0,
        f.full.1,
        f.bytes_read,
        f.requests,
        t0.elapsed().as_secs_f64()
    );
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_decode::geo::specular_gis::fixtures::{self, GeoGrid};
    use specular_remote::testing::{ServerOptions, TestResponse, TestServer};

    /// A COG served with range support (a local test server): the level that fits comes
    /// back as a GeoTIFF that opens georeferenced, every request a range.
    #[test]
    fn fetches_a_level_over_http() {
        assert!(is_remote_cog("https://h/x/dem.tif?sig=1"));
        assert!(is_remote_cog("http://h/x/DEM.TIFF"));
        assert!(!is_remote_cog("/data/dem.tif"));
        assert!(!is_remote_cog("https://h/x.copc.laz"));
        let (w, h) = (900usize, 700usize);
        let values = fixtures::dem_values(w, h, false);
        let grid = GeoGrid {
            epsg: 32632,
            origin: (500_000.0, 5_200_000.0),
            pixel: (10.0, 10.0),
        };
        let tif = fixtures::geotiff_f32(&values, w, h, grid, None, 256);
        let served = tif.clone();
        let server = TestServer::start(ServerOptions::default(), move |r| {
            TestResponse::ranged(r, &served, "\"v1\"")
        })
        .unwrap();
        let out = std::env::temp_dir().join(format!("specular-cog-{}", std::process::id()));
        let client = Client::new(ClientConfig::for_tests()).unwrap();
        let cancel = CancelToken::new();
        let url = server.url("/dem.tif");
        let (path, f) = fetch_to(&client, &url, &out, &cancel, &mut |_| {}).unwrap();
        assert_eq!((f.level, f.width, f.height), (0, 900, 700));
        assert!(f.requests >= 2);
        let s = specular_decode::Registry::default()
            .open_path(&path)
            .unwrap();
        let g = s.geo().expect("georeferenced");
        assert_eq!(g.crs.as_ref().and_then(|c| c.epsg), Some(32632));
        assert_eq!(g.map_at(0.0, 0.0), (500_000.0, 5_200_000.0));
        assert!(server.log().iter().all(|r| r.header("range").is_some()));
        let _ = std::fs::remove_dir_all(&out);
    }

    /// A COG given by its address is one "Image tiles" row in the download indicator: its
    /// host only, every range request counted, bytes and a full bar when done.
    #[test]
    fn a_fetch_is_a_row_in_downloads() {
        use specular_core::net::{Registry, State, What};
        let (w, h) = (300usize, 200usize);
        let values = fixtures::dem_values(w, h, false);
        let grid = GeoGrid {
            epsg: 32632,
            origin: (500_000.0, 5_200_000.0),
            pixel: (10.0, 10.0),
        };
        let tif = fixtures::geotiff_f32(&values, w, h, grid, None, 128);
        let server = TestServer::start(ServerOptions::default(), move |r| {
            TestResponse::ranged(r, &tif, "\"v1\"")
        })
        .unwrap();
        let out = std::env::temp_dir().join(format!("specular-cog-row-{}", std::process::id()));
        let reg = Registry::new();
        let client = Client::new(ClientConfig::for_tests())
            .unwrap()
            .reporting_to(reg.clone());
        let url = server.url("/dem.tif?X-Amz-Signature=SECRET");
        let path = fetch_arg_with(&client, &url, &out).unwrap();
        assert!(path.exists());
        let snap = reg.snapshot();
        assert_eq!(snap.len(), 1, "{snap:?}");
        let s = &snap[0];
        assert_eq!(s.label.what, What::ImageTiles);
        assert_eq!(s.label.host, specular_core::net::host_of(&url));
        assert!(!format!("{s:?}").contains("SECRET"));
        assert_eq!(s.state, State::Done);
        assert_eq!(s.requests as usize, server.log().len());
        assert!(s.requests >= 2 && s.failed == 0, "{s:?}");
        assert!(s.bytes > 0 && s.fraction == Some(1.0), "{s:?}");
        let _ = std::fs::remove_dir_all(&out);
    }
}
