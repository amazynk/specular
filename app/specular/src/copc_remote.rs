//! A COPC point cloud on an HTTP server (`specular https://…/cloud.copc.laz`): its octree
//! levels are read by range requests, coarse to fine, as long as they fit the point budget,
//! and kept as a smaller COPC file in the cache, which then opens like a local one.

use std::path::PathBuf;

use specular_decode::specular_points::{self, Fetched, RangeRead};
use specular_remote::{CancelToken, Client, ClientConfig, Request};

/// Points fetched at most (whole levels; the root level always).
pub const MAX_POINTS: u64 = 16_000_000;
/// Compressed bytes fetched at most.
pub const MAX_BYTES: u64 = 512 << 20;

/// Whether a command-line argument names a remote COPC file.
pub fn is_remote_copc(arg: &str) -> bool {
    let l = arg.trim().to_ascii_lowercase();
    (l.starts_with("https://") || l.starts_with("http://"))
        && l.split(['?', '#'])
            .next()
            .is_some_and(|p| p.ends_with(".copc.laz"))
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
            206 if r.body.len() as u64 == len => Ok(r.body),
            206 => Err("the server sent a short range".into()),
            200 => Err("the server does not answer range requests".into()),
            s => Err(format!("the server answered {s}")),
        }
    }
}

/// The levels fetched from `url`, as a COPC file in `dir`; `progress(level, points)`.
pub fn fetch_to(
    client: &Client,
    url: &str,
    dir: &std::path::Path,
    cancel: &CancelToken,
    progress: &mut dyn FnMut(u32, u64),
) -> Result<(PathBuf, Fetched), String> {
    let src = Http {
        client,
        url,
        cancel,
    };
    let mut f = specular_points::fetch_levels(&src, MAX_POINTS, MAX_BYTES, progress)
        .map_err(|e| e.to_string())?;
    let name = url
        .split(['?', '#'])
        .next()
        .and_then(|p| p.rsplit('/').next())
        .filter(|n| !n.is_empty())
        .unwrap_or("cloud.copc.laz");
    let key = specular_remote::DiskCache::url_key(url);
    let key = key.rsplit('/').next().unwrap_or("x").to_owned();
    let dir = dir.join(key);
    std::fs::create_dir_all(&dir).map_err(|e| crate::error_text::io_reason(&e))?;
    let path = dir.join(specular_remote::cache::sanitize(name));
    std::fs::write(&path, std::mem::take(&mut f.image))
        .map_err(|e| crate::error_text::io_reason(&e))?;
    Ok((path, f))
}

/// For a COPC address given on the command line: fetches the levels that fit into the
/// cache (on the window's worker, so "Downloads" shows it) and returns the local file to
/// open, or why not.
pub fn fetch_arg(url: &str) -> Result<PathBuf, String> {
    let client = Client::new(ClientConfig::default()).map_err(|e| e.to_string())?;
    let dir = specular_settings::Locations::detect()
        .cache_dir
        .join("copc");
    fetch_arg_with(&client, url, &dir)
}

/// [`fetch_arg`] with the client and cache folder given: every range request is counted
/// in one "Point cloud from <host>" row of the client's download registry.
pub fn fetch_arg_with(
    client: &Client,
    url: &str,
    dir: &std::path::Path,
) -> Result<PathBuf, String> {
    let client = client.tracked(specular_core::net::What::PointCloud);
    let cancel = CancelToken::new();
    let t0 = std::time::Instant::now();
    let (p, f) = fetch_to(&client, url, dir, &cancel, &mut |_, _| {})?;
    specular_log::info!(
        target: specular_log::IO,
        "copc: {} of {} levels, {} of {} points, {} bytes in {} requests, {:.1} s",
        f.levels,
        f.file_levels,
        f.points,
        f.total,
        f.bytes_read,
        f.requests,
        t0.elapsed().as_secs_f64()
    );
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_remote::testing::{ServerOptions, TestResponse, TestServer};

    /// A COPC file served with range support: what comes back opens with the same points;
    /// with the root level only it is the coarse cloud. Uses SPECULAR_POINTS_COPC (skips
    /// without it).
    #[test]
    fn streams_levels_over_http() {
        assert!(is_remote_copc("https://h/x/autzen.copc.laz?sig=1"));
        assert!(!is_remote_copc("/data/autzen.copc.laz"));
        assert!(!is_remote_copc("https://h/x.laz"));
        let Some(dir) = std::env::var_os("SPECULAR_POINTS_COPC") else {
            eprintln!("SPECULAR_POINTS_COPC not set; skipping");
            return;
        };
        let file = PathBuf::from(dir).join("lone-star.copc.laz");
        let Ok(data) = std::fs::read(&file) else {
            return;
        };
        let served = data.clone();
        let server = TestServer::start(ServerOptions::default(), move |r| {
            TestResponse::ranged(r, &served, "\"v1\"")
        })
        .unwrap();
        let out = std::env::temp_dir().join(format!("specular-copc-{}", std::process::id()));
        let client = Client::new(ClientConfig::for_tests()).unwrap();
        let cancel = CancelToken::new();
        let url = server.url("/lone-star.copc.laz");
        let mut levels = Vec::new();
        let (path, f) = fetch_to(&client, &url, &out, &cancel, &mut |l, _| levels.push(l)).unwrap();
        assert_eq!(levels, (0..f.file_levels).collect::<Vec<_>>());
        let limits = specular_points::Limits::default();
        let local =
            specular_points::read(specular_points::PointFormat::Laz, &data, &limits).unwrap();
        let fetched = specular_points::read(
            specular_points::PointFormat::Laz,
            &std::fs::read(&path).unwrap(),
            &limits,
        )
        .unwrap();
        assert_eq!(fetched.len(), local.len());
        // Every request was a range.
        assert!(server.log().iter().all(|r| r.header("range").is_some()));
        let _ = std::fs::remove_dir_all(&out);
    }

    /// A COPC address is one "Point cloud" row in the download indicator with each range
    /// request counted, even when the file turns out not to be COPC (a LAS 1.4 header
    /// without the COPC record: refused after two requests).
    #[test]
    fn a_fetch_is_a_row_in_downloads() {
        use specular_core::net::{Registry, State, What};
        let mut las = vec![0u8; 375];
        las[..4].copy_from_slice(b"LASF");
        (las[24], las[25]) = (1, 4);
        las[94..96].copy_from_slice(&375u16.to_le_bytes());
        las[96..100].copy_from_slice(&375u32.to_le_bytes());
        let server = TestServer::start(ServerOptions::default(), move |r| {
            TestResponse::ranged(r, &las, "\"v1\"")
        })
        .unwrap();
        let out = std::env::temp_dir().join(format!("specular-copc-row-{}", std::process::id()));
        let reg = Registry::new();
        let client = Client::new(ClientConfig::for_tests())
            .unwrap()
            .reporting_to(reg.clone());
        let url = server.url("/cloud.copc.laz");
        let e = fetch_arg_with(&client, &url, &out).unwrap_err();
        assert!(!e.is_empty());
        let snap = reg.snapshot();
        assert_eq!(snap.len(), 1, "{snap:?}");
        let s = &snap[0];
        assert_eq!(s.label.what, What::PointCloud);
        assert_eq!(s.label.host, specular_core::net::host_of(&url));
        assert_eq!(s.state, State::Done);
        assert_eq!((s.requests, s.finished), (2, 2), "{s:?}");
        assert_eq!(s.bytes, 750);
        let _ = std::fs::remove_dir_all(&out);
    }
}
