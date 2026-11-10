//! Screenshot tests of a remote OME-Zarr image (generated here, served by a loopback
//! server): the first, coarse picture the viewer shows while the chunks of the level it
//! needs come in, with the download card ("Zarr chunks from <host>", n of m requests), in
//! English and in Hebrew (right to left), at 1× and 1.5×; and the sharp picture once the
//! full-resolution chunks are in. Rendered offscreen on lavapipe and compared with the
//! PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular zarr::shots`, then review and
//! commit the rewritten PNGs.

use std::sync::Arc;
use std::time::{Duration, Instant};

use specular_core::keys::KeyStore;
use specular_core::net::{Label, Registry, What};
use specular_core::{CancelToken, Orientation, Raster, Source};
use specular_decode::zarr_url_dec::{self, ZarrLink};
use specular_decode::{DecodeRequest, Session};
use specular_remote::testing::{ServerOptions, TestServer};
use specular_remote::zarr_fixture::{self, Layout, Spec};
use specular_remote::{Client, ClientConfig, RemoteStore, StoreOptions};
use specular_render::{ImageRenderer, PxRect, RenderOptions, View};
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;

const SIZE: (f32, f32) = (1200.0, 620.0);
/// Shown in place of the loopback server's address, whose port changes from run to run.
const HOST: &str = "https://data.example.org/idr/6001240.ome.zarr";

/// The fixture over HTTP, opened as the viewer opens a link.
fn session() -> (TestServer, Arc<dyn Session>) {
    let spec = Spec {
        layout: Layout::V3Sharded,
        size: 512,
        chunk: 64,
        levels: 4,
        planes: 1,
        ..Spec::default()
    };
    let files = Arc::new(zarr_fixture::ome_zarr(&spec));
    let server = TestServer::start(
        ServerOptions::default(),
        zarr_fixture::handler(files, "/img.zarr/"),
    )
    .unwrap();
    let url = server.url("/img.zarr/");
    let store: Arc<dyn KeyStore> = Arc::new(
        RemoteStore::open(
            Client::new(ClientConfig::for_tests()).unwrap(),
            &url,
            &specular_remote::s3::MapEnv::default(),
            None,
            StoreOptions::default(),
        )
        .unwrap(),
    );
    let link = ZarrLink {
        url,
        path: String::new(),
        name: String::new(),
    };
    let source = Arc::new(Source::from_vec("6001240.zarrurl", link.to_json()));
    let s = zarr_url_dec::open_store(source, store, "").expect("the remote image opens");
    (server, s)
}

/// The download card's activity: 16 chunk requests begun, 6 done, a second in.
fn downloads() -> specular_widgets::Downloads {
    let reg = Registry::new();
    let t0 = Instant::now();
    let reqs: Vec<_> = (0..16)
        .map(|_| reg.begin_at(Label::new(What::ZarrChunks, HOST), t0))
        .collect();
    for (k, mut r) in reqs.into_iter().enumerate() {
        if k < 6 {
            r.head(Some(32_768), 0);
            r.add(32_768);
            r.finish_at(Ok(()), t0 + Duration::from_millis(400));
        } else {
            std::mem::forget(r);
        }
    }
    let acts = reg.snapshot_at(t0 + Duration::from_secs(1));
    assert_eq!(acts.len(), 1, "one activity for the store's chunks");
    let v = super::super::net::view(&acts, &specular_i18n::I18n::global().localizer());
    assert!(!v.rows[0].title.contains("idr"), "only the host is shown");
    v
}

fn shoot(
    h: &mut Harness,
    goldens: &mut Goldens,
    name: &str,
    raster: &Raster,
    card: bool,
    scale: f32,
) {
    let (w, ht) = (512u32, 512u32);
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, w, ht, None);
    renderer.upload_overview(&h.gpu, &mut img, raster);
    let view_of = |size: (u32, u32)| {
        let zoom = 0.86 * size.1 as f32 / ht as f32;
        View::fixed(
            (w, ht),
            Orientation::NORMAL,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: size.0 as f32,
                h: size.1 as f32,
            },
            zoom,
            (w as f32 / 2.0, ht as f32 / 2.0),
        )
    };
    let settings = specular_settings::Settings::default();
    let store = specular_settings::SettingsStore::in_memory();
    let hud = specular_widgets::HudStats::default();
    let keymap = specular_input::Keymap::default();
    let st = specular_widgets::ViewerState {
        actions: specular_input::ActionState {
            image: specular_input::ImageState::Ready,
            count: 1,
            pages: 1,
            ..specular_input::ActionState::default()
        },
        has_image: true,
        index: 0,
        count: 1,
        zoom: 0.86,
        at_fit: true,
        fullscreen: false,
        file_name: Some("6001240.zarrurl"),
        dims: Some((w, ht)),
        loading: false,
        error: None,
        truncated: false,
        metadata: None,
        hud: &hud,
        keymap: &keymap,
        store: &store,
        settings: &settings,
        overlays: specular_widgets::Overlays::default(),
        playback: None,
        motion: None,
        spread: None,
        pages: None,
    };
    let mut c = specular_widgets::Chrome::new();
    c.reduce_motion = true;
    c.set_size(SIZE.0, SIZE.1);
    c.poke(1.0);
    if card {
        c.set_downloads(Some(downloads()));
    }
    let pal = Palette::dark();
    let shot = h.render_gpu(
        SIZE,
        scale,
        |gpu, enc, target, size| {
            let view = view_of(size);
            renderer.render(
                gpu,
                enc,
                target,
                size,
                Some(&img),
                &view,
                &RenderOptions::default(),
            );
        },
        |dl, text| {
            let mut warm = specular_gfx::DrawList::new(scale);
            c.draw(&mut warm, text, &st, &pal, 1.0);
            c.draw(dl, text, &st, &pal, 1.0);
        },
    );
    goldens.check(&format!("{name}-{scale}x"), &shot);
}

/// Opening: the coarsest level that fills the view (64×64 of 512×512, the first decode)
/// under the download card; then the sharp picture from level 0.
#[test]
fn remote_ome_zarr_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-zarr",
    );
    let (server, s) = session();
    let cancel = CancelToken::new();
    let opened = server.requests();
    let coarse = s.decode(&DecodeRequest::full(8), &cancel).unwrap();
    assert_eq!((coarse.width, coarse.height), (64, 64));
    let coarse_requests = server.requests() - opened;
    let sharp = s.decode(&DecodeRequest::full(1), &cancel).unwrap();
    assert!(
        server.requests() - opened > coarse_requests * 4,
        "the coarse picture took a few requests ({coarse_requests}), the sharp one many"
    );
    for scale in [1.0, 1.5] {
        shoot(
            &mut h,
            &mut goldens,
            "zarr-remote-coarse-dark",
            &coarse,
            true,
            scale,
        );
    }
    shoot(
        &mut h,
        &mut goldens,
        "zarr-remote-sharp-dark",
        &sharp,
        false,
        1.0,
    );
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        for scale in [1.0, 1.5] {
            shoot(
                &mut h,
                &mut goldens,
                "zarr-remote-coarse-he",
                &coarse,
                true,
                scale,
            );
        }
    });
    goldens.finish();
}
