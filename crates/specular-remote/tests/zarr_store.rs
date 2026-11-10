//! Remote Zarr stores against servers written here (loopback only): plain HTTP with
//! ranges, and an S3 emulation (path-style and virtual-hosted, anonymous and SigV4-signed,
//! the signature checked by the server). Lazy fetching, cancel, and errors.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use specular_core::keys::{KeyError, KeyStore, Want};
use specular_remote::s3::{self, Credentials, MapEnv, S3Location};
use specular_remote::testing::{ServerOptions, TestRequest, TestResponse, TestServer};
use specular_remote::zarr_fixture::{self, Files, Layout, Spec};
use specular_remote::{Client, ClientConfig, DiskCache, RemoteStore, Secret, StoreOptions};

fn client() -> Client {
    Client::new(ClientConfig::for_tests()).unwrap()
}

fn spec() -> Spec {
    Spec {
        size: 128,
        chunk: 32,
        levels: 2,
        ..Spec::default()
    }
}

fn env(vars: &[(&str, &str)]) -> MapEnv {
    MapEnv {
        vars: vars
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
        home: None,
    }
}

fn http_store(server: &TestServer, path: &str) -> RemoteStore {
    RemoteStore::open(
        client(),
        &server.url(path),
        &env(&[]),
        None,
        StoreOptions::default(),
    )
    .unwrap()
}

#[test]
fn http_reads_only_what_is_asked() {
    let files = Arc::new(zarr_fixture::ome_zarr(&spec()));
    let server = TestServer::start(
        ServerOptions::default(),
        zarr_fixture::handler(files.clone(), "/d/img.zarr/"),
    )
    .unwrap();
    let store = http_store(&server, "/d/img.zarr");
    assert_eq!(
        store.get(".zattrs", None).unwrap().as_deref(),
        Some(files[".zattrs"].as_slice())
    );
    assert_eq!(store.get("zarr.json", None).unwrap(), None, "404 is absent");
    let wants: Vec<Want> = ["0/0/0/0/0", "0/0/0/0/1", "1/1/2/1/1"]
        .iter()
        .map(|k| Want::whole(*k))
        .collect();
    store.prefetch(&wants, None).unwrap();
    let before = server.requests();
    for w in &wants {
        assert_eq!(
            store.get(&w.key, None).unwrap().as_deref(),
            Some(files[&w.key].as_slice())
        );
    }
    assert_eq!(
        server.requests(),
        before,
        "prefetched values come from memory"
    );
    let asked: Vec<String> = server.log().iter().map(|r| r.path().to_owned()).collect();
    assert_eq!(asked.len(), 5, "{asked:?}");
    assert!(asked.iter().all(|p| !p.contains("0/0/1/")), "{asked:?}");
    // Ranges: a sharded store's index and an inner chunk.
    let v3 = Arc::new(zarr_fixture::ome_zarr(&Spec {
        layout: Layout::V3Sharded,
        ..spec()
    }));
    let s3 = TestServer::start(
        ServerOptions::default(),
        zarr_fixture::handler(v3.clone(), "/v3/"),
    )
    .unwrap();
    let store = http_store(&s3, "/v3/");
    let shard = &v3["0/c/0/0/0/0"];
    let (idx, size) = store.get_suffix("0/c/0/0/0/0", 68, None).unwrap().unwrap();
    assert_eq!(size, shard.len() as u64);
    assert_eq!(idx, &shard[shard.len() - 68..]);
    let part = store
        .get_range("0/c/0/0/0/0", 2048, 100, None)
        .unwrap()
        .unwrap();
    assert_eq!(part, &shard[2048..2148]);
    let log = s3.log();
    assert_eq!(log[0].header("range"), Some("bytes=-68"));
    assert_eq!(log[1].header("range"), Some("bytes=2048-2147"));
}

#[test]
fn query_strings_go_with_every_request_but_not_into_the_location() {
    let files = Arc::new(zarr_fixture::ome_zarr(&spec()));
    let f = files.clone();
    let server = TestServer::start(ServerOptions::default(), move |r| {
        if r.param("sig").as_deref() != Some("abc") {
            return TestResponse::status(403);
        }
        zarr_fixture::handler(f.clone(), "/x.zarr/")(r)
    })
    .unwrap();
    let store = RemoteStore::open(
        client(),
        &server.url("/x.zarr?sig=abc"),
        &env(&[]),
        None,
        StoreOptions::default(),
    )
    .unwrap();
    assert!(store.get(".zattrs", None).unwrap().is_some());
    assert!(store.get("0/0/0/0/0", None).unwrap().is_some());
    assert!(!store.location().contains("sig"));
    assert!(store.location().ends_with("/x.zarr/"));
}

#[test]
fn servers_without_ranges_are_sliced() {
    let files = Arc::new(zarr_fixture::ome_zarr(&spec()));
    let f = files.clone();
    let server = TestServer::start(ServerOptions::default(), move |r| {
        match f.get(r.path().trim_start_matches('/')) {
            Some(d) => TestResponse::ok(d.clone()),
            None => TestResponse::status(404),
        }
    })
    .unwrap();
    let store = http_store(&server, "/");
    let d = &files["0/0/0/0/0"];
    assert_eq!(
        store.get_range("0/0/0/0/0", 10, 20, None).unwrap().unwrap(),
        &d[10..30]
    );
    let (tail, n) = store.get_suffix("0/0/0/0/0", 16, None).unwrap().unwrap();
    assert_eq!((tail.as_slice(), n), (&d[d.len() - 16..], d.len() as u64));
}

#[test]
fn errors_say_what_happened() {
    let server = TestServer::start(ServerOptions::default(), |r: &TestRequest| match r.path() {
        "/z/.zattrs" => TestResponse::ok(b"{}".to_vec()),
        "/z/denied" => TestResponse::status(403),
        "/z/broken" => TestResponse::status(500),
        "/z/slow" => TestResponse::ok(b"late".to_vec()).delay(Duration::from_secs(3)),
        _ => TestResponse::status(404),
    })
    .unwrap();
    let store = http_store(&server, "/z/");
    assert_eq!(store.get("absent", None), Ok(None));
    assert_eq!(store.get("denied", None), Err(KeyError::Denied));
    assert_eq!(store.get("broken", None), Err(KeyError::Status(500)));
    let impatient = Client::new(ClientConfig {
        response_timeout: Duration::from_millis(300),
        retries: 0,
        ..ClientConfig::for_tests()
    })
    .unwrap();
    let store = RemoteStore::open(
        impatient,
        &server.url("/z/"),
        &env(&[]),
        None,
        StoreOptions::default(),
    )
    .unwrap();
    assert_eq!(store.get("slow", None), Err(KeyError::Timeout));
    // Nothing listens on a port just freed.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let store = RemoteStore::open(
        client(),
        &format!("http://127.0.0.1:{port}/z/"),
        &env(&[]),
        None,
        StoreOptions::default(),
    )
    .unwrap();
    assert!(matches!(
        store.get(".zattrs", None),
        Err(KeyError::Offline(_))
    ));
    for e in [KeyError::Denied, KeyError::Status(503), KeyError::Timeout] {
        let text = e.to_string();
        assert!(!text.is_empty() && !text.contains("Status("), "{text}");
    }
}

#[test]
fn a_view_change_cancels_the_fetches() {
    let files = Arc::new(zarr_fixture::ome_zarr(&spec()));
    let server = TestServer::start(
        ServerOptions {
            latency: Duration::from_millis(400),
            ..ServerOptions::default()
        },
        zarr_fixture::handler(files, "/"),
    )
    .unwrap();
    let store = Arc::new(
        RemoteStore::open(
            client(),
            &server.url("/"),
            &env(&[]),
            None,
            StoreOptions {
                concurrency: 2,
                ..StoreOptions::default()
            },
        )
        .unwrap(),
    );
    let wants: Vec<Want> = (0..4)
        .flat_map(|y| (0..4).map(move |x| Want::whole(format!("0/0/0/{y}/{x}"))))
        .collect();
    let flag = Arc::new(AtomicBool::new(false));
    let f = flag.clone();
    let stopper = specular_thread::spawn("stop", move || {
        std::thread::sleep(Duration::from_millis(100));
        f.store(true, Ordering::Relaxed);
    });
    let t0 = Instant::now();
    assert_eq!(
        store.prefetch(&wants, Some(&flag)),
        Err(KeyError::Cancelled)
    );
    assert!(
        t0.elapsed() < Duration::from_millis(350),
        "{:?}",
        t0.elapsed()
    );
    stopper.join().unwrap();
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        server.requests() <= 2,
        "only the first two had started: {}",
        server.requests()
    );
}

#[test]
fn chunks_come_back_from_the_disk_cache() {
    let files = Arc::new(zarr_fixture::ome_zarr(&spec()));
    let server =
        TestServer::start(ServerOptions::default(), zarr_fixture::handler(files, "/")).unwrap();
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("zarr-disk-{}", std::process::id()));
    let disk = DiskCache::open(&dir, 64 << 20).unwrap();
    let opts = StoreOptions {
        disk: Some(disk),
        ..StoreOptions::default()
    };
    let a = RemoteStore::open(client(), &server.url("/"), &env(&[]), None, opts.clone()).unwrap();
    let first = a.get("0/1/2/1/1", None).unwrap().unwrap();
    a.get(".zattrs", None).unwrap();
    let n = server.requests();
    let b = RemoteStore::open(client(), &server.url("/"), &env(&[]), None, opts).unwrap();
    assert_eq!(b.get("0/1/2/1/1", None).unwrap().unwrap(), first);
    assert_eq!(server.requests(), n, "the chunk came from disk");
    b.get(".zattrs", None).unwrap();
    assert_eq!(server.requests(), n + 1, "metadata is always asked for");
    let _ = std::fs::remove_dir_all(&dir);
}

// ----- S3 ------------------------------------------------------------------------------

const KEY: &str = "AKIDTESTEXAMPLE";
const SECRET: &str = "test/secret/key+example";

struct Bucket {
    name: &'static str,
    prefix: &'static str,
    files: Files,
    public: bool,
    region: &'static str,
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Whether a request's SigV4 signature is right for the test key.
fn signed_ok(r: &TestRequest, region: &str) -> Option<bool> {
    let auth = r.header("authorization")?;
    let field = |name: &str| {
        auth.split([',', ' '])
            .find_map(|p| p.trim().strip_prefix(name))
            .map(str::to_owned)
    };
    let credential = field("Credential=")?;
    let signed = field("SignedHeaders=")?;
    if !credential.starts_with(&format!("{KEY}/")) {
        return Some(false);
    }
    let headers: Vec<(String, String)> = signed
        .split(';')
        .map(|h| (h.to_owned(), r.header(h).unwrap_or("").to_owned()))
        .collect();
    let path = r.target.split('?').next().unwrap_or("/");
    let date = r.header("x-amz-date")?;
    let creds = Credentials {
        access_key: KEY.into(),
        secret: Secret::new(SECRET),
        session_token: None,
    };
    let (_, _, expect) = s3::sign_parts(
        &s3::ToSign {
            method: &r.method,
            path,
            query: &r.query(),
            headers: &headers,
            payload_hash: r.header("x-amz-content-sha256").unwrap_or(""),
            region,
            service: "s3",
            amz_date: date,
        },
        &creds,
    );
    Some(expect == auth && signed.contains("host"))
}

fn s3_server(b: Bucket) -> TestServer {
    let b = Arc::new(b);
    TestServer::start(ServerOptions::default(), move |r| {
        // A proxy tunnel: the requests then come on this connection, as to the bucket.
        if r.method == "CONNECT" {
            return TestResponse::ok(Vec::new());
        }
        let mut plain = r.clone();
        if let Some(rest) = plain.target.strip_prefix("http://") {
            plain.target = rest.find('/').map_or("/".into(), |i| rest[i..].to_owned());
        }
        let authorised = match signed_ok(&plain, b.region) {
            Some(true) => true,
            Some(false) => return TestResponse::status(403),
            None => false,
        };
        if !b.public && !authorised {
            return TestResponse::status(403);
        }
        // Through a proxy the target is the absolute URL.
        let mut r = r.clone();
        if let Some(rest) = r.target.strip_prefix("http://") {
            r.target = rest.find('/').map_or("/".into(), |i| rest[i..].to_owned());
        }
        let r = &r;
        let host = r.header("host").unwrap_or("");
        let virtual_host = host.starts_with(&format!("{}.", b.name));
        let path = percent_decode(r.path());
        let rest = if virtual_host {
            path.strip_prefix('/')
        } else {
            path.strip_prefix(&format!("/{}", b.name))
                .map(|p| p.strip_prefix('/').unwrap_or(p))
        };
        let Some(rest) = rest else {
            return TestResponse::status(404);
        };
        if rest.is_empty() && r.param("list-type").as_deref() == Some("2") {
            if !authorised {
                return TestResponse::status(403);
            }
            let prefix = r.param("prefix").unwrap_or_default();
            let mut dirs: Vec<String> = b
                .files
                .keys()
                .filter_map(|k| {
                    let k = format!("{}{k}", b.prefix);
                    let tail = k.strip_prefix(&prefix)?;
                    let (d, _) = tail.split_once('/')?;
                    Some(format!("{prefix}{d}/"))
                })
                .collect();
            dirs.dedup();
            let xml: String = dirs
                .iter()
                .map(|d| format!("<CommonPrefixes><Prefix>{d}</Prefix></CommonPrefixes>"))
                .collect();
            return TestResponse::ok(
                format!(
                    "<ListBucketResult><IsTruncated>false</IsTruncated>{xml}</ListBucketResult>"
                )
                .into_bytes(),
            );
        }
        match rest.strip_prefix(b.prefix).and_then(|k| b.files.get(k)) {
            Some(d) => TestResponse::ranged(r, d, "\"s3\""),
            // Without list rights a missing key is "access denied".
            None if !authorised => TestResponse::status(403),
            None => TestResponse::status(404),
        }
    })
    .unwrap()
}

fn base(server: &TestServer) -> String {
    server.url("").trim_end_matches('/').to_owned()
}

#[test]
fn public_bucket_is_read_unsigned_path_style() {
    let server = s3_server(Bucket {
        name: "zarr-pub",
        prefix: "data/img.zarr/",
        files: zarr_fixture::ome_zarr(&spec()),
        public: true,
        region: "eu-west-1",
    });
    let e = env(&[
        ("AWS_ENDPOINT_URL", &base(&server)),
        ("AWS_REGION", "eu-west-1"),
    ]);
    let store = RemoteStore::open(
        client(),
        "s3://zarr-pub/data/img.zarr",
        &e,
        None,
        StoreOptions::default(),
    )
    .unwrap();
    assert!(store.get(".zattrs", None).unwrap().is_some());
    assert_eq!(store.get("zarr.json", None).unwrap(), None);
    assert!(
        store.was_denied(),
        "a 403 for a missing key is taken as absent"
    );
    assert!(
        server
            .log()
            .iter()
            .all(|r| r.header("authorization").is_none())
    );
    assert_eq!(server.log()[0].path(), "/zarr-pub/data/img.zarr/.zattrs");
    assert_eq!(store.location(), "s3://zarr-pub/data/img.zarr/");
}

#[test]
fn private_bucket_is_signed_after_a_refusal() {
    let server = s3_server(Bucket {
        name: "zarr-priv",
        prefix: "img.zarr/",
        files: zarr_fixture::ome_zarr(&spec()),
        public: false,
        region: "us-west-2",
    });
    let e = env(&[
        ("AWS_ENDPOINT_URL_S3", &base(&server)),
        ("AWS_REGION", "us-west-2"),
    ]);
    // Without credentials the bucket stays shut.
    let store = RemoteStore::open(
        client(),
        "s3://zarr-priv/img.zarr/",
        &e,
        None,
        StoreOptions::default(),
    )
    .unwrap();
    assert_eq!(store.get(".zattrs", None).unwrap(), None);
    assert!(store.refused());
    // With the configured ones (here as a test's environment): unsigned once, then signed.
    let mut e = e;
    e.vars.push(("AWS_ACCESS_KEY_ID".into(), KEY.into()));
    e.vars.push(("AWS_SECRET_ACCESS_KEY".into(), SECRET.into()));
    let creds = Credentials::configured(&e);
    assert!(creds.is_some());
    let n0 = server.requests();
    let store = RemoteStore::open(
        client(),
        "s3://zarr-priv/img.zarr/",
        &e,
        creds,
        StoreOptions::default(),
    )
    .unwrap();
    assert!(store.get(".zattrs", None).unwrap().is_some());
    assert!(store.is_signing());
    let (idx, _) = store.get_suffix("0/0/0/0/0", 16, None).unwrap().unwrap();
    assert_eq!(idx.len(), 16);
    assert!(store.get_range("1/1/2/0/0", 0, 8, None).unwrap().is_some());
    assert_eq!(store.size("0/0/0/0/0", None).unwrap(), Some(2048));
    assert_eq!(
        store.get("0/9/9/9/9", None).unwrap(),
        None,
        "signed: 404 is absent"
    );
    assert_eq!(store.subdirs("", None).unwrap(), ["0", "1"]);
    let log = server.log();
    let mine = &log[n0 as usize..];
    assert!(mine[0].header("authorization").is_none(), "anonymous first");
    assert!(
        mine[1..]
            .iter()
            .all(|r| r.header("authorization").is_some())
    );
    // The secret never goes over the wire.
    assert!(
        mine.iter()
            .all(|r| r.headers.iter().all(|(_, v)| !v.contains(SECRET)))
    );
}

/// A virtual-hosted bucket by its real AWS address, reached through a proxy (the test
/// server) so nothing leaves the machine: the bucket in the host, signed for that host.
#[test]
fn virtual_hosted_bucket() {
    let server = s3_server(Bucket {
        name: "zarr-vh",
        prefix: "img.zarr/",
        files: zarr_fixture::ome_zarr(&spec()),
        public: false,
        region: "us-west-2",
    });
    let proxied = Client::new(ClientConfig {
        proxy: Some(base(&server)),
        ..ClientConfig::for_tests()
    })
    .unwrap();
    let url = "http://zarr-vh.s3.us-west-2.amazonaws.com/img.zarr";
    let loc = S3Location::parse(url, &env(&[])).unwrap();
    assert!(!loc.path_style);
    let creds = Credentials {
        access_key: KEY.into(),
        secret: Secret::new(SECRET),
        session_token: None,
    };
    let store = RemoteStore::open(
        proxied,
        url,
        &env(&[]),
        Some(creds),
        StoreOptions::default(),
    )
    .unwrap();
    assert!(store.get(".zattrs", None).unwrap().is_some());
    assert!(store.get_range("0/0/0/0/0", 0, 64, None).unwrap().is_some());
    let r = server.log().pop().unwrap();
    assert_eq!(r.path(), "/img.zarr/0/0/0/0/0");
    assert_eq!(r.header("host"), Some("zarr-vh.s3.us-west-2.amazonaws.com"));
    assert!(r.header("authorization").is_some());
}
