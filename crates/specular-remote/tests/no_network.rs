//! The no-network switch (`specular_core::net::off`) at the client's choke point: with it
//! on, every way this crate can reach a server (plain requests, streams, downloads to a
//! file, resumable downloads, remote files, Zarr stores over HTTP and S3) is refused before
//! a socket opens, and nothing reaches the download registry. Checked against a loopback
//! server that counts the connections it accepts: zero.
//!
//! Its own test binary (the switch is process-wide), with one test.

use std::ops::ControlFlow;
use std::sync::Arc;

use specular_core::keys::{KeyError, KeyStore, Want};
use specular_core::net::off::{self, By};
use specular_remote::s3::MapEnv;
use specular_remote::testing::{ServerOptions, TestResponse, TestServer};
use specular_remote::{
    Auth, CancelToken, Client, ClientConfig, Error, RemoteFile, RemoteStore, Request, Resumer,
    StoreOptions,
};

#[test]
fn nothing_connects_while_the_network_is_off() {
    let body = Arc::new(vec![7u8; 4096]);
    let b = body.clone();
    let server = TestServer::start(ServerOptions::default(), move |r| {
        TestResponse::ranged(r, &b, "\"v1\"")
    })
    .unwrap();
    let reg = specular_core::net::global();
    let client = Client::new(ClientConfig::for_tests()).unwrap();
    // Tracked like the app's clients, so a request that got through would be listed.
    let tracked = client.tracked(specular_core::net::What::Data);
    let cancel = CancelToken::new();
    let dir = std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/test-tmp/remote-no-network"
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    off::set(By::Setting, true);
    let refused0 = off::refused();
    let url = server.url("/f.bin");
    let off_err = |e: Error| assert!(e.is_network_off() && e.is_offline(), "{e:?}");

    off_err(tracked.head(&Request::head(&url), &cancel).unwrap_err());
    off_err(tracked.get(&Request::get(&url), &cancel).unwrap_err());
    off_err(
        tracked
            .post(&Request::post(&url, "text/plain", b"x".to_vec()), &cancel)
            .unwrap_err(),
    );
    off_err(
        tracked
            .get_stream(&Request::get(&url), &cancel, |_, _| {
                ControlFlow::Continue(())
            })
            .unwrap_err(),
    );
    off_err(
        tracked
            .download_to(&Request::get(&url), &cancel, &dir.join("f.bin"), |_| {})
            .unwrap_err(),
    );
    assert!(!dir.join("f.bin").exists());
    let resumer = Resumer::new(tracked.clone(), Some(dir.join("validators")));
    off_err(
        resumer
            .open(&Request::get(&url), 100, &cancel)
            .err()
            .unwrap(),
    );
    let rf = RemoteFile::new(tracked.clone(), url.clone(), Auth::None, None, 1 << 20);
    off_err(rf.read_at(0, 16, &cancel).unwrap_err());
    let env = MapEnv::default();
    for store_url in [server.url("/img.zarr"), "s3://bucket/img.zarr".to_owned()] {
        let store = RemoteStore::open(
            tracked.clone(),
            &store_url,
            &env,
            None,
            StoreOptions::default(),
        )
        .unwrap();
        assert_eq!(store.get(".zattrs", None), Err(KeyError::NetworkOff));
        assert_eq!(
            store.prefetch(&[Want::whole("0/0.0")], None),
            Err(KeyError::NetworkOff)
        );
    }

    assert_eq!(
        server
            .stats()
            .connections
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    assert_eq!(server.requests(), 0);
    assert!(
        reg.snapshot().is_empty(),
        "the Downloads indicator would show"
    );
    assert!(off::refused() > refused0);

    // The same harness sees a connection once the network is on again.
    off::set(By::Setting, false);
    assert_eq!(
        client.get(&Request::get(&url), &cancel).unwrap().body,
        *body
    );
    assert!(
        server
            .stats()
            .connections
            .load(std::sync::atomic::Ordering::Relaxed)
            >= 1
    );
    let _ = std::fs::remove_dir_all(&dir);
}
