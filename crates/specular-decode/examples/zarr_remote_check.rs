//! Dev-only check of remote Zarr against real servers (never run by the tests):
//! `cargo run -p specular-decode --example zarr_remote_check -- URL…`
//! (or `tools/corpus/check-remote-zarr.sh`). For each address: the images listed, the first
//! opened, the coarsest picture and a 512×512 region at full resolution decoded, with the
//! requests and times each took.

use std::sync::Arc;
use std::time::Instant;

use specular_core::keys::KeyStore;
use specular_core::{CancelToken, Source};
use specular_decode::zarr_url_dec::{self, ZarrLink};
use specular_decode::{DecodeRequest, Rect};
use specular_remote::s3::{Credentials, ProcessEnv, S3Location};
use specular_remote::{Client, ClientConfig, RemoteStore, StoreOptions};

fn main() {
    let urls: Vec<String> = std::env::args().skip(1).collect();
    if urls.is_empty() {
        eprintln!("usage: zarr_remote_check URL…");
        std::process::exit(2);
    }
    let mut failed = 0;
    for url in &urls {
        if let Err(e) = check(url) {
            println!("{url}: FAILED: {e}");
            failed += 1;
        }
    }
    std::process::exit(i32::from(failed > 0));
}

fn check(url: &str) -> Result<(), String> {
    let env = ProcessEnv;
    let creds = S3Location::parse(url, &env).and_then(|_| Credentials::configured(&env));
    let client = Client::new(ClientConfig::default()).map_err(|e| e.to_string())?;
    let store = Arc::new(
        RemoteStore::open(client, url, &env, creds, StoreOptions::default())
            .map_err(|e| e.to_string())?,
    );
    let t0 = Instant::now();
    let images = zarr_url_dec::images(store.as_ref(), None).map_err(|e| e.to_string())?;
    println!(
        "{url}: {} image(s) [{}] in {:.0} ms, {} requests",
        images.len(),
        images
            .iter()
            .take(4)
            .map(|i| if i.name.is_empty() {
                "root"
            } else {
                i.name.as_str()
            })
            .collect::<Vec<_>>()
            .join(", "),
        t0.elapsed().as_secs_f64() * 1000.0,
        store.requests()
    );
    let first = images.first().ok_or("no images")?;
    let link = ZarrLink {
        url: store.location(),
        path: first.path.clone(),
        name: first.name.clone(),
    };
    let source = Arc::new(Source::from_vec("check.zarrurl", link.to_json()));
    let t = Instant::now();
    let n = store.requests();
    let s =
        zarr_url_dec::open_store(source, store.clone(), &first.path).map_err(|e| e.to_string())?;
    let info = s.info();
    println!(
        "  open: {}×{} {:?} '{}' in {:.0} ms, {} requests",
        info.width,
        info.height,
        info.format,
        info.coding,
        t.elapsed().as_secs_f64() * 1000.0,
        store.requests() - n
    );
    let cancel = CancelToken::new();
    let coarse = *info.scales.last().unwrap_or(&1);
    let t = Instant::now();
    let n = store.requests();
    let r = s
        .decode(&DecodeRequest::full(coarse), &cancel)
        .map_err(|e| e.to_string())?;
    println!(
        "  coarse 1/{coarse}: {}×{} in {:.0} ms, {} requests",
        r.width,
        r.height,
        t.elapsed().as_secs_f64() * 1000.0,
        store.requests() - n
    );
    let (w, h) = (info.width.min(512), info.height.min(512));
    let req = DecodeRequest {
        region: Some(Rect {
            x: (info.width - w) / 2,
            y: (info.height - h) / 2,
            w,
            h,
        }),
        ..DecodeRequest::full(1)
    };
    let t = Instant::now();
    let n = store.requests();
    let r = s.decode(&req, &cancel).map_err(|e| e.to_string())?;
    println!(
        "  region 1:1 {}×{} in {:.0} ms, {} requests{}",
        r.width,
        r.height,
        t.elapsed().as_secs_f64() * 1000.0,
        store.requests() - n,
        if r.truncated { " (truncated)" } else { "" }
    );
    Ok(())
}
