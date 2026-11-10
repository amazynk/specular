//! OME-Zarr and plain Zarr read over HTTP from a loopback server (no outside server):
//! every decode compared with the same store read from a local folder (the oracle), and
//! the requests checked: only the chunks of the level and region asked for.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use specular_core::keys::{KeyError, KeyStore};
use specular_core::{CancelToken, Source};
use specular_decode::zarr_url_dec::{self, ZarrLink};
use specular_decode::{DecodeError, DecodeRequest, Rect, Registry, Session};
use specular_remote::testing::{ServerOptions, TestResponse, TestServer};
use specular_remote::zarr_fixture::{self, Files, Layout, Spec};
use specular_remote::{Client, ClientConfig, RemoteStore, StoreOptions};

fn spec(layout: Layout) -> Spec {
    Spec {
        layout,
        size: 256,
        chunk: 32,
        levels: 3,
        ..Spec::default()
    }
}

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("zr-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The store written to a local `.ome.zarr` folder (the oracle).
fn local(files: &Files, name: &str) -> (PathBuf, Arc<dyn Session>) {
    let root = tmp(name).join("img.ome.zarr");
    for (k, v) in files {
        let p = root.join(k);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, v).unwrap();
    }
    let s = Registry::default()
        .open_path(&root)
        .expect("local store opens");
    (root, s)
}

fn store(server: &TestServer, path: &str) -> Arc<dyn KeyStore> {
    let c = Client::new(ClientConfig::for_tests()).unwrap();
    let env = specular_remote::s3::MapEnv::default();
    Arc::new(RemoteStore::open(c, &server.url(path), &env, None, StoreOptions::default()).unwrap())
}

fn link_source(url: &str, path: &str) -> Arc<Source> {
    let l = ZarrLink {
        url: url.into(),
        path: path.into(),
        name: String::new(),
    };
    Arc::new(Source::from_vec("image.zarrurl", l.to_json()))
}

fn open_remote(
    server: &TestServer,
    url_path: &str,
    path: &str,
) -> Result<Arc<dyn Session>, DecodeError> {
    zarr_url_dec::open_store(
        link_source(&server.url(url_path), path),
        store(server, url_path),
        path,
    )
}

fn mismatches(a: &specular_core::Raster, b: &specular_core::Raster) -> usize {
    assert_eq!((a.width, a.height, a.origin), (b.width, b.height, b.origin));
    a.planes[0]
        .data
        .iter()
        .zip(&b.planes[0].data)
        .filter(|(x, y)| x != y)
        .count()
}

fn chunk_paths(server: &TestServer, from: usize) -> Vec<String> {
    server.log()[from..]
        .iter()
        .map(|r| r.path().to_owned())
        .filter(|p| {
            !p.ends_with(".zattrs")
                && !p.ends_with(".zarray")
                && !p.ends_with(".zgroup")
                && !p.ends_with("zarr.json")
        })
        .collect()
}

fn progressive(layout: Layout) {
    let files = Arc::new(zarr_fixture::ome_zarr(&spec(layout)));
    let (_, oracle) = local(&files, &format!("{layout:?}"));
    let server = TestServer::start(
        ServerOptions::default(),
        zarr_fixture::handler(files.clone(), "/img.zarr/"),
    )
    .unwrap();
    let s = open_remote(&server, "/img.zarr/", "").expect("remote store opens");
    assert_eq!((s.info().width, s.info().height), (256, 256));
    let opened = server.requests() as usize;
    // First the whole image, coarse: level 2 (64×64, 2×2 chunks a plane).
    let cancel = CancelToken::new();
    let coarse = s.decode(&DecodeRequest::full(4), &cancel).unwrap();
    assert_eq!(
        mismatches(
            &coarse,
            &oracle.decode(&DecodeRequest::full(4), &cancel).unwrap()
        ),
        0
    );
    let first = chunk_paths(&server, opened);
    assert!(!first.is_empty());
    assert!(
        first.iter().all(|p| p.starts_with("/img.zarr/2/")),
        "coarse level only: {first:?}"
    );
    // Then a region at full resolution: only the level-0 chunks under it.
    let n = server.requests() as usize;
    let req = DecodeRequest {
        region: Some(Rect {
            x: 40,
            y: 70,
            w: 50,
            h: 30,
        }),
        ..DecodeRequest::full(1)
    };
    let fine = s.decode(&req, &cancel).unwrap();
    assert_eq!(mismatches(&fine, &oracle.decode(&req, &cancel).unwrap()), 0);
    let second = chunk_paths(&server, n);
    assert!(
        second.iter().all(|p| p.starts_with("/img.zarr/0/")),
        "{second:?}"
    );
    match layout {
        // x 40..90 → chunks 1..2, y 70..100 → 2..3: 4 chunks for each of 2 channels.
        Layout::V2 => {
            assert_eq!(second.len(), 8, "{second:?}");
            assert!(
                second.iter().all(|p| p.ends_with("/2/1")
                    || p.ends_with("/2/2")
                    || p.ends_with("/3/1")
                    || p.ends_with("/3/2")),
                "{second:?}"
            );
        }
        // The same chunks inside shards 0..1 × 1: an index (suffix) per shard, then ranges.
        Layout::V3Sharded => {
            let log = &server.log()[n..];
            assert!(
                log.iter().all(|r| r.header("range").is_some()),
                "only ranges"
            );
            let suffixes = log
                .iter()
                .filter(|r| r.header("range").is_some_and(|v| v.starts_with("bytes=-")))
                .count();
            assert_eq!(suffixes, 4, "two shards a channel");
            assert_eq!(log.len() - suffixes, 8);
        }
    }
    // Asked again: from memory.
    let m = server.requests();
    s.decode(&req, &cancel).unwrap();
    assert_eq!(server.requests(), m);
    // Channels and planes as the local reader gives them.
    let sci = s.science().unwrap();
    let names: Vec<&str> = sci.axes.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["C", "Z"]);
    let p = s.nd_plane(&[1, 2], &cancel).unwrap().unwrap();
    let q = oracle.nd_plane(&[1, 2], &cancel).unwrap().unwrap();
    assert_eq!(p.values.len(), 256 * 256);
    let bad = p
        .values
        .iter()
        .zip(&q.values)
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(bad, 0);
    assert_eq!(
        p.values[(100 * 256 + 30) as usize],
        f32::from(zarr_fixture::sample(1, 2, 100, 30, 256))
    );
    let loc = sci
        .keywords
        .iter()
        .find(|k| k.0 == "Location")
        .map(|k| k.1.clone());
    assert_eq!(loc.as_deref(), Some(server.url("/img.zarr/").as_str()));
}

#[test]
fn v2_opens_coarse_first_and_reads_only_visible_chunks() {
    progressive(Layout::V2);
}

#[test]
fn v3_sharded_reads_indexes_then_inner_chunks() {
    progressive(Layout::V3Sharded);
}

#[test]
fn a_moved_view_stops_its_downloads() {
    let files = Arc::new(zarr_fixture::ome_zarr(&spec(Layout::V2)));
    let server = TestServer::start(ServerOptions::default(), {
        let h = zarr_fixture::handler(files, "/");
        move |r| {
            let resp = h(r);
            if r.path().starts_with("/0/") {
                resp.delay(Duration::from_millis(500))
            } else {
                resp
            }
        }
    })
    .unwrap();
    let s = open_remote(&server, "/", "").unwrap();
    let cancel = CancelToken::new();
    let c = cancel.clone();
    let t = specular_thread::spawn("cancel", move || {
        std::thread::sleep(Duration::from_millis(100));
        c.cancel();
    });
    let t0 = Instant::now();
    let r = s.decode(&DecodeRequest::full(1), &cancel);
    assert!(matches!(r, Err(DecodeError::Cancelled)), "{:?}", r.err());
    assert!(
        t0.elapsed() < Duration::from_millis(450),
        "{:?}",
        t0.elapsed()
    );
    t.join().unwrap();
}

fn kind_of(e: &DecodeError) -> Option<(std::io::ErrorKind, KeyError)> {
    let DecodeError::Io(io) = e else { return None };
    let k = io.get_ref()?.downcast_ref::<KeyError>()?.clone();
    Some((io.kind(), k))
}

#[test]
fn errors_are_readable() {
    let files = Arc::new(zarr_fixture::ome_zarr(&spec(Layout::V2)));
    let f = files.clone();
    let server = TestServer::start(ServerOptions::default(), move |r| match r.path() {
        p if p.starts_with("/private/") => TestResponse::status(403),
        p if p.starts_with("/broken/0/0/") => TestResponse::status(500),
        p => match p
            .strip_prefix("/broken/")
            .or_else(|| p.strip_prefix("/ok/"))
            .and_then(|k| f.get(k))
        {
            Some(d) => TestResponse::ranged(r, d, "\"x\""),
            None => TestResponse::status(404),
        },
    })
    .unwrap();
    let e = open_remote(&server, "/nothing/", "").err().unwrap();
    assert_eq!(
        kind_of(&e),
        Some((std::io::ErrorKind::NotFound, KeyError::Status(404)))
    );
    let e = open_remote(&server, "/private/", "").err().unwrap();
    assert_eq!(
        kind_of(&e),
        Some((std::io::ErrorKind::PermissionDenied, KeyError::Denied))
    );
    let s = open_remote(&server, "/broken/", "").unwrap();
    let e = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .err()
        .unwrap();
    assert!(e.to_string().contains("500"), "{e}");
    // No opener registered for a link opened from disk (the decode host's case).
    let dir = tmp("noopener");
    let p = dir.join("x.zarrurl");
    std::fs::write(
        &p,
        ZarrLink {
            url: server.url("/ok/"),
            ..Default::default()
        }
        .to_json(),
    )
    .unwrap();
    assert!(Registry::default().open_path(&p).is_err());
}

#[test]
fn plates_and_series_list_their_images() {
    let small = Spec {
        size: 64,
        chunk: 32,
        levels: 2,
        planes: 1,
        ..Spec::default()
    };
    let plate = Arc::new(zarr_fixture::plate(&small));
    let server = TestServer::start(
        ServerOptions::default(),
        zarr_fixture::handler(plate, "/p.zarr/"),
    )
    .unwrap();
    let st = store(&server, "/p.zarr/");
    let images = zarr_url_dec::images(st.as_ref(), None).unwrap();
    let names: Vec<&str> = images.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(
        names,
        ["A1, field 1", "A1, field 2", "B2, field 1", "B2, field 2"]
    );
    assert_eq!(images[3].path, "B/2/1/");
    let s = open_remote(&server, "/p.zarr/", &images[3].path).unwrap();
    assert_eq!(s.info().width, 64);
    let bf = Arc::new(zarr_fixture::bioformats2raw(&small, 3));
    let server =
        TestServer::start(ServerOptions::default(), zarr_fixture::handler(bf, "/b/")).unwrap();
    let images = zarr_url_dec::images(store(&server, "/b/").as_ref(), None).unwrap();
    let paths: Vec<&str> = images.iter().map(|i| i.path.as_str()).collect();
    assert_eq!(paths, ["0/", "1/", "2/"]);
    assert_eq!(images[1].name, "Series 2");
    // One image: the root.
    let one = Arc::new(zarr_fixture::ome_zarr(&small));
    let server =
        TestServer::start(ServerOptions::default(), zarr_fixture::handler(one, "/")).unwrap();
    let images = zarr_url_dec::images(store(&server, "/").as_ref(), None).unwrap();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].path, "");
}

#[test]
fn plain_zarr_needs_consolidated_metadata_over_http() {
    // A group with one 2-D array, no OME metadata.
    let mut files = Files::new();
    files.insert(".zgroup".into(), br#"{"zarr_format":2}"#.to_vec());
    files.insert(".zattrs".into(), br#"{"title":"plain"}"#.to_vec());
    let zarray = br#"{"zarr_format":2,"shape":[40,50],"chunks":[16,16],"dtype":"<f4","compressor":null,"fill_value":0,"order":"C","filters":null}"#;
    files.insert("t/.zarray".into(), zarray.to_vec());
    for cy in 0..3 {
        for cx in 0..4 {
            let mut b = Vec::new();
            for y in 0..16 {
                for x in 0..16 {
                    b.extend_from_slice(
                        &((cy * 16 + y) as f32 * 100.0 + (cx * 16 + x) as f32).to_le_bytes(),
                    );
                }
            }
            files.insert(format!("t/{cy}.{cx}"), b);
        }
    }
    let bare = Arc::new(files.clone());
    let server =
        TestServer::start(ServerOptions::default(), zarr_fixture::handler(bare, "/")).unwrap();
    let e = open_remote(&server, "/", "").err().unwrap();
    assert!(
        matches!(&e, DecodeError::Unsupported(m) if m.contains("consolidated")),
        "{e:?}"
    );
    let mut consolidated = files.clone();
    consolidated.insert(
        ".zmetadata".into(),
        format!(
            r#"{{"zarr_consolidated_format":1,"metadata":{{".zgroup":{{"zarr_format":2}},".zattrs":{{"title":"plain"}},"t/.zarray":{}}}}}"#,
            String::from_utf8_lossy(zarray)
        )
        .into_bytes(),
    );
    let server = TestServer::start(
        ServerOptions::default(),
        zarr_fixture::handler(Arc::new(consolidated), "/"),
    )
    .unwrap();
    let s = open_remote(&server, "/", "").unwrap();
    assert_eq!((s.info().width, s.info().height), (50, 40));
    let p = s.nd_plane(&[], &CancelToken::new()).unwrap().unwrap();
    assert_eq!(p.values[(33 * 50 + 47) as usize], 3347.0);
}
