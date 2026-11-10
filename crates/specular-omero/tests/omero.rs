//! Against the in-process fake OMERO.web (no external servers): sign-in, expiry, paging,
//! imgData, exact tiles at every level, edge tiles, progressive opening, planes, channel
//! strings, thumbnails, errors and the decoder hook. The live IDR check runs only with
//! `SPECULAR_OMERO_LIVE=1`.

use std::sync::Arc;

use specular_core::{CancelToken, Source};
use specular_decode::{DecodeRequest, Format, PageLabel, Rect, Session};
use specular_omero::fake::{self, Access, FakeConfig, FakeRender, FakeServer};
use specular_omero::session::open_with;
use specular_omero::{
    Descriptor, Encoding, Kind, Listing, Omero, OmeroError, Render, Rgba, descriptor, tiles,
};
use specular_remote::testing::ServerOptions;
use specular_remote::{Client, ClientConfig, DiskCache, Secret};

fn client() -> Client {
    Client::new(ClientConfig::for_tests()).unwrap()
}

fn server(cfg: FakeConfig) -> FakeServer {
    FakeServer::start(cfg, ServerOptions::default()).unwrap()
}

fn connect(srv: &FakeServer, login: Option<(&str, &str)>) -> Arc<Omero> {
    let o = Omero::new(client(), &srv.base()).unwrap();
    o.connect(login.map(|(u, p)| (u, Secret::new(p))), &CancelToken::new())
        .unwrap();
    o
}

fn private() -> FakeConfig {
    FakeConfig {
        access: Access::Private,
        account: Some(("alice".into(), "s3cret &=?".into())),
        prefix: "/omero".into(),
        ..FakeConfig::synthetic(3)
    }
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("omero-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// RGBA of the fake's level pixels.
fn expected(
    img: u64,
    level: u32,
    rect: (u32, u32, u32, u32),
    z: u32,
    t: u32,
    render: &FakeRender,
) -> Vec<u8> {
    let cfg = FakeConfig::synthetic(3);
    let i = cfg.images.iter().find(|i| i.id == img).unwrap();
    fake::region(i, level, rect, z, t, render)
        .chunks_exact(3)
        .flat_map(|p| [p[0], p[1], p[2], 255])
        .collect()
}

fn rgba_of(r: &specular_core::Raster) -> Vec<u8> {
    let p = &r.planes[0];
    let mut out = Vec::new();
    for y in 0..r.height as usize {
        out.extend_from_slice(&p.data[y * p.stride..y * p.stride + r.width as usize * 4]);
    }
    out
}

fn mismatches(a: &[u8], b: &[u8]) -> usize {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).filter(|(x, y)| x != y).count()
}

fn descriptor(srv: &FakeServer, image: u64, encoding: Option<Encoding>) -> Descriptor {
    Descriptor {
        server: specular_omero::api::check_url(&srv.base()).unwrap(),
        image,
        name: "x".into(),
        encoding,
        ..Descriptor::default()
    }
}

fn session(
    srv: &FakeServer,
    o: &Arc<Omero>,
    d: Descriptor,
    cache: Option<Arc<DiskCache>>,
) -> Arc<specular_omero::OmeroSession> {
    let src = Arc::new(Source::from_vec(d.file_name(), d.to_json()));
    open_with(o.clone(), cache, src, d, &CancelToken::new())
        .unwrap_or_else(|e| panic!("open {}: {e}", srv.base()))
}

#[test]
fn signs_in_with_a_csrf_token_and_session_cookie() {
    let srv = server(private());
    let o = connect(&srv, Some(("alice", "s3cret &=?")));
    assert_eq!(o.logins(), 1);
    assert_eq!(o.user().as_deref(), Some("alice"));
    let ec = o.event_context().unwrap();
    assert_eq!((ec.user_id, ec.group_name.as_str()), (52, "Fake lab"));
    let page = o
        .list(Listing::Projects, 0, 50, &CancelToken::new())
        .unwrap();
    assert_eq!(page.items.len(), 2);
    // The login was a form POST with the token in the header and cookie, and a referer.
    let login = srv
        .server
        .log()
        .into_iter()
        .find(|r| r.path().ends_with("/login/"))
        .unwrap();
    assert_eq!(login.method, "POST");
    assert_eq!(login.header("x-csrftoken"), Some(fake::TOKEN));
    assert!(login.header("referer").unwrap().starts_with("http://"));
    // The password never reaches a URL.
    assert!(
        srv.server
            .log()
            .iter()
            .all(|r| !r.target.contains("s3cret"))
    );

    let bad = Omero::new(client(), &srv.base()).unwrap();
    let e = bad
        .connect(Some(("alice", Secret::new("wrong"))), &CancelToken::new())
        .unwrap_err();
    assert_eq!(e, OmeroError::LoginRefused);
    assert!(e.needs_sign_in());
}

#[test]
fn a_server_setting_the_csrf_cookie_works_too() {
    let srv = server(FakeConfig {
        csrf_cookie: true,
        ..private()
    });
    let o = connect(&srv, Some(("alice", "s3cret &=?")));
    assert!(o.list(Listing::Screens, 0, 10, &CancelToken::new()).is_ok());
}

#[test]
fn private_servers_want_a_sign_in() {
    let srv = server(private());
    let o = connect(&srv, None);
    let e = o
        .list(Listing::Projects, 0, 10, &CancelToken::new())
        .unwrap_err();
    assert_eq!(e, OmeroError::SignInRequired);
    let e = o.img_data(fake::TILED, &CancelToken::new()).unwrap_err();
    assert_eq!(e, OmeroError::SignInRequired);
}

#[test]
fn public_servers_start_a_session_from_the_web_client() {
    let srv = server(FakeConfig {
        access: Access::Public,
        ..FakeConfig::synthetic(0)
    });
    let o = connect(&srv, None);
    let page = o
        .list(Listing::Projects, 0, 10, &CancelToken::new())
        .unwrap();
    assert_eq!(page.total, 2);
    assert_eq!(srv.requested("/webclient/").len(), 1);
    // Later requests reuse the public session.
    o.list(Listing::Screens, 0, 10, &CancelToken::new())
        .unwrap();
    assert_eq!(srv.requested("/webclient/").len(), 1);
    assert_eq!(o.logins(), 0);
}

#[test]
fn expired_sessions_sign_in_again_once() {
    let srv = server(private());
    let o = connect(&srv, Some(("alice", "s3cret &=?")));
    srv.expire_sessions();
    let d = o.img_data(fake::TILED, &CancelToken::new()).unwrap();
    assert_eq!(d.id, fake::TILED);
    assert_eq!(o.logins(), 2);
    // Several requests failing together sign in once.
    srv.expire_sessions();
    std::thread::scope(|s| {
        for _ in 0..4 {
            specular_thread::spawn_scoped(s, "omero-test", || {
                o.list(Listing::Projects, 0, 5, &CancelToken::new())
                    .unwrap();
            });
        }
    });
    assert_eq!(o.logins(), 3);
    // Without the password the user is asked again.
    o.forget_password();
    srv.expire_sessions();
    let e = o
        .list(Listing::Projects, 0, 5, &CancelToken::new())
        .unwrap_err();
    assert_eq!(e, OmeroError::Expired);
    assert!(e.needs_sign_in());
}

#[test]
fn listings_page_with_offset_limit_and_total() {
    let srv = server(FakeConfig {
        prefix: "/omero".into(),
        ..FakeConfig::synthetic(7)
    });
    let o = connect(&srv, None);
    let c = CancelToken::new();
    let projects = o.list(Listing::Projects, 0, 100, &c).unwrap();
    assert_eq!(projects.items[0].children, Some(2));
    assert_eq!(projects.items[0].owner.as_deref(), Some("Ada Lovelace"));
    let datasets = o
        .list(
            Listing::Datasets {
                project: fake::PROJECT,
            },
            0,
            100,
            &c,
        )
        .unwrap();
    assert_eq!(datasets.items[0].kind, Kind::Dataset);
    assert_eq!(datasets.items[0].children, Some(9));
    let images = Listing::Images {
        dataset: fake::DATASET,
    };
    let p = o.list(images, 0, 4, &c).unwrap();
    assert_eq!(
        (p.items.len(), p.offset, p.total, p.more()),
        (4, 0, 9, true)
    );
    let last = o.list(images, 8, 4, &c).unwrap();
    assert_eq!((last.items.len(), last.more()), (1, false));
    let all = o.list_all(images, 1000, &c).unwrap();
    assert_eq!(all.len(), 9);
    assert_eq!(all[0].image.as_ref().unwrap().size, [1000, 700, 1, 3, 1]);
    assert_eq!(
        all[0].image.as_ref().unwrap().acquired.as_deref(),
        Some("2016-04-01 12:30:00")
    );
    assert!(
        srv.requested("/api/v0/m/datasets/11/images/?offset=8&limit=4")
            .len()
            == 1
    );

    let screens = o.list(Listing::Screens, 0, 10, &c).unwrap();
    let plates = o
        .list(
            Listing::Plates {
                screen: screens.items[0].id,
            },
            0,
            10,
            &c,
        )
        .unwrap();
    let wells = o
        .list(
            Listing::Wells {
                plate: plates.items[0].id,
            },
            0,
            10,
            &c,
        )
        .unwrap();
    let names: Vec<&str> = wells.items.iter().map(|w| w.name.as_str()).collect();
    assert_eq!(names, ["A1", "A2", "A3", "B1", "B2", "B3"]);
    assert_eq!(wells.items[4].image_id(), Some(254));
    assert_eq!(o.image(fake::STACK, &c).unwrap().name, "stack.ome.tif");
}

#[test]
fn img_data_describes_tiled_and_whole_images() {
    let srv = server(FakeConfig::synthetic(0));
    let o = connect(&srv, None);
    let c = CancelToken::new();
    let t = o.img_data(fake::TILED, &c).unwrap();
    assert!(t.tiled);
    assert_eq!((t.tile, t.levels), ((256, 256), 3));
    assert_eq!(t.level_size(2), (250, 175));
    assert_eq!(t.channels.len(), 3);
    assert_eq!(t.project.as_ref().unwrap().1, "Synthetic project");
    assert_eq!(t.pixel_size[0], Some(0.325));
    let s = o.img_data(fake::STACK, &c).unwrap();
    assert!(!s.tiled);
    assert_eq!(s.size, [64, 48, 3, 2, 2]);
    assert_eq!((s.default_z, s.planes()), (1, 6));
    assert_eq!(s.channels[1].window.start, 100.0);
}

#[test]
fn tiles_are_exact_at_every_level_and_edge() {
    let srv = server(FakeConfig::synthetic(0));
    let o = connect(&srv, None);
    let s = session(
        &srv,
        &o,
        descriptor(&srv, fake::TILED, Some(Encoding::Png)),
        None,
    );
    assert_eq!(s.info().format, Format::Omero);
    assert_eq!((s.info().width, s.info().height), (1000, 700));
    assert_eq!(s.info().scales, &[1, 2, 4, 8, 16, 32]);
    let all = FakeRender::all(3);
    let mut bad = 0;
    // Full resolution, across tile seams and the right and bottom edge tiles.
    for (x, y, w, h) in [(250, 250, 20, 20), (900, 600, 100, 100), (0, 0, 300, 10)] {
        let req = DecodeRequest {
            region: Some(Rect { x, y, w, h }),
            ..DecodeRequest::full(1)
        };
        let r = s.decode(&req, &CancelToken::new()).unwrap();
        assert_eq!((r.origin, r.width, r.height), ((x, y), w, h));
        bad += mismatches(
            &rgba_of(&r),
            &expected(fake::TILED, 0, (x, y, w, h), 0, 0, &all),
        );
    }
    // Whole levels.
    for (scale, level, (w, h)) in [(2, 1, (500, 350)), (4, 2, (250, 175))] {
        let r = s
            .decode(&DecodeRequest::full(scale), &CancelToken::new())
            .unwrap();
        assert_eq!((r.width, r.height, r.scale), (w, h, scale));
        bad += mismatches(
            &rgba_of(&r),
            &expected(fake::TILED, level, (0, 0, w, h), 0, 0, &all),
        );
    }
    // Beyond the coarsest level: box-filtered from it.
    let r = s
        .decode(&DecodeRequest::full(16), &CancelToken::new())
        .unwrap();
    assert_eq!((r.width, r.height), (63, 44));
    let l2 = Rgba {
        width: 250,
        height: 175,
        data: expected(fake::TILED, 2, (0, 0, 250, 175), 0, 0, &all),
    };
    bad += mismatches(&rgba_of(&r), &tiles::reduce(&l2, 4, 63, 44).data);
    assert_eq!(bad, 0, "{bad} mismatching samples");
    // Only tiles of the right sizes were asked for; the server cut the edges.
    for t in srv.requested("/webgateway/render_image_region/") {
        assert!(t.contains(",256,256&c=1%7C"), "{t}");
        assert!(t.ends_with("&m=c&format=png"), "{t}");
    }
}

#[test]
fn opening_is_progressive() {
    let srv = server(FakeConfig::synthetic(0));
    let o = connect(&srv, None);
    let s = session(&srv, &o, descriptor(&srv, fake::TILED, None), None);
    // The viewer's first picture: the coarsest scale.
    let coarsest = *s.info().scales.last().unwrap();
    let r = s
        .decode(&DecodeRequest::full(coarsest), &CancelToken::new())
        .unwrap();
    assert_eq!((r.width, r.height), (32, 22));
    let asked = srv.requested("/webgateway/render_image_region/");
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert!(asked[0].contains("tile=2,0,0,256,256"));
    assert!(asked[0].ends_with("format=jpeg&q=0.9"));
    // Then a region at full resolution: only the tiles under it.
    let req = DecodeRequest {
        region: Some(Rect {
            x: 300,
            y: 300,
            w: 100,
            h: 100,
        }),
        ..DecodeRequest::full(1)
    };
    let r = s.decode(&req, &CancelToken::new()).unwrap();
    let asked = srv.requested("/webgateway/render_image_region/");
    assert_eq!(asked.len(), 2);
    assert!(asked[1].contains("tile=0,1,1,256,256"));
    // JPEG tiles are close to the exact pixels.
    let exact = expected(
        fake::TILED,
        0,
        (300, 300, 100, 100),
        0,
        0,
        &FakeRender::all(3),
    );
    let got = rgba_of(&r);
    let mean = got
        .iter()
        .zip(&exact)
        .map(|(a, b)| u64::from(a.abs_diff(*b)))
        .sum::<u64>() as f64
        / got.len() as f64;
    assert!(mean < 3.0, "mean error {mean}");
}

#[test]
fn tiles_go_through_the_cache_and_cancel() {
    let srv = server(FakeConfig::synthetic(0));
    let o = connect(&srv, None);
    let dir = temp_dir("cache");
    let cache = DiskCache::open(&dir, 64 << 20).unwrap();
    let d = descriptor(&srv, fake::TILED, Some(Encoding::Png));
    let s = session(&srv, &o, d.clone(), Some(cache.clone()));
    let a = s
        .decode(&DecodeRequest::full(2), &CancelToken::new())
        .unwrap();
    let n = srv.requested("/webgateway/render_image_region/").len();
    assert_eq!(n, 4);
    // A new session (as after reopening) reads the cache.
    let s2 = session(&srv, &o, d, Some(cache));
    let b = s2
        .decode(&DecodeRequest::full(2), &CancelToken::new())
        .unwrap();
    assert_eq!(srv.requested("/webgateway/render_image_region/").len(), n);
    assert_eq!(rgba_of(&a), rgba_of(&b));
    let cancel = CancelToken::new();
    cancel.cancel();
    let e = s.decode(&DecodeRequest::full(1), &cancel).unwrap_err();
    assert!(
        matches!(e, specular_decode::DecodeError::Cancelled),
        "{e:?}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn planes_are_pages_and_channels_are_rendered_as_asked() {
    let srv = server(FakeConfig::synthetic(0));
    let o = connect(&srv, None);
    let mut d = descriptor(&srv, fake::STACK, None);
    let s = session(&srv, &o, d.clone(), None);
    assert_eq!(s.page_count(), 6);
    assert_eq!(s.page_index(), 1, "the server's default Z");
    assert_eq!(
        s.page_label(),
        Some(PageLabel::Plane {
            z: Some((1, 3)),
            t: Some((0, 2))
        })
    );
    let p = s.open_page(4).unwrap();
    assert_eq!(p.page_index(), 4);
    let full = p
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    let all = FakeRender::all(2);
    assert_eq!(
        mismatches(
            &rgba_of(&full),
            &expected(fake::STACK, 0, (0, 0, 64, 48), 1, 1, &all)
        ),
        0
    );
    // A region and a smaller scale reuse the whole plane.
    let req = DecodeRequest {
        region: Some(Rect {
            x: 4,
            y: 4,
            w: 8,
            h: 8,
        }),
        ..DecodeRequest::full(2)
    };
    let r = p.decode(&req, &CancelToken::new()).unwrap();
    assert_eq!((r.width, r.height, r.origin), (8, 8, (4, 4)));
    assert_eq!(srv.requested("/webgateway/render_image/102/1/1/").len(), 1);
    assert!(srv.requested("/webgateway/render_image/")[0].ends_with("&format=png"));
    // Channel 2 off: green is gone, and the channel string says so.
    let mut render = Render::of(&o.img_data(fake::STACK, &CancelToken::new()).unwrap());
    assert!(render.toggle(1));
    d.render = Some(render);
    d.z = Some(0);
    let s = session(&srv, &o, d.clone(), None);
    assert_eq!(s.page_index(), 0);
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    let off = FakeRender {
        active: vec![true, false],
        greyscale: false,
    };
    assert_eq!(
        mismatches(
            &rgba_of(&r),
            &expected(fake::STACK, 0, (0, 0, 64, 48), 0, 0, &off)
        ),
        0
    );
    let last = srv
        .requested("/webgateway/render_image/102/0/0/")
        .pop()
        .unwrap();
    assert!(
        last.contains("c=1%7C0%3A4000%24FF0000%2C-2%7C100%3A4100%2400FF00&m=c"),
        "{last}"
    );
    // The descriptor round-trips the rendering.
    assert_eq!(Descriptor::parse(&d.to_json()), Some(d));
}

#[test]
fn summaries_carry_the_omero_facts() {
    let srv = server(FakeConfig::synthetic(0));
    let o = connect(&srv, None);
    let mut d = descriptor(&srv, fake::TILED, None);
    d.context.dataset = Some("Gradients".into());
    let s = session(&srv, &o, d, None);
    let sum = s.science().unwrap();
    assert!(sum.raster_only);
    assert_eq!(sum.vendor.as_deref(), Some("OMERO"));
    assert_eq!(sum.pixel.as_ref().unwrap().x.step, 0.325);
    assert_eq!(sum.levels, [(1000, 700), (500, 350), (250, 175)]);
    let f = sum.omero.as_ref().unwrap();
    assert_eq!(f.image_id, fake::TILED);
    assert_eq!(f.name, "tiled slide.svs");
    assert_eq!(f.project.as_deref(), Some("Synthetic project"));
    assert_eq!(f.dataset.as_deref(), Some("Gradients"));
    assert_eq!(f.owner.as_deref(), Some("Ada Lovelace"));
    assert_eq!(f.acquired.as_deref(), Some("2016-04-01 12:30:00"));
    assert_eq!(f.size, [1000, 700, 1, 3, 1]);
    assert_eq!(f.channels.len(), 3);
    assert_eq!(f.channels[1].colour, [0, 255, 0]);
    assert_eq!(f.tiles, Some((256, 256, 3)));
    assert_eq!(f.encoding, "JPEG");
}

#[test]
fn thumbnails_decode_and_cache() {
    let srv = server(FakeConfig::synthetic(0));
    let o = connect(&srv, None);
    let dir = temp_dir("thumbs");
    let cache = DiskCache::open(&dir, 8 << 20).unwrap();
    let c = CancelToken::new();
    let t = tiles::thumbnail(&o, Some(&cache), fake::TILED, 96, &c).unwrap();
    assert_eq!((t.width, t.height), (96, 67));
    let t = tiles::thumbnail(&o, Some(&cache), fake::STACK, 96, &c).unwrap();
    assert_eq!((t.width, t.height), (96, 72));
    tiles::thumbnail(&o, Some(&cache), fake::STACK, 96, &c).unwrap();
    assert_eq!(srv.requested("/webgateway/render_thumbnail/").len(), 2);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn errors_are_told_apart() {
    let srv = server(FakeConfig::synthetic(0));
    let o = connect(&srv, None);
    let c = CancelToken::new();
    assert_eq!(o.img_data(9999, &c).unwrap_err(), OmeroError::NotFound);
    assert_eq!(
        o.list(Listing::Datasets { project: 77 }, 0, 10, &c)
            .unwrap_err(),
        OmeroError::NotFound
    );
    srv.fail_next(&[404]);
    assert_eq!(
        o.img_data(fake::TILED, &c).unwrap_err(),
        OmeroError::NotFound
    );
    srv.fail_next(&[500]);
    assert_eq!(
        o.img_data(fake::TILED, &c).unwrap_err(),
        OmeroError::Status(500)
    );
    // Refused twice (before and after starting a public session): a sign-in is needed.
    srv.fail_next(&[401, 401]);
    assert_eq!(
        o.img_data(fake::TILED, &c).unwrap_err(),
        OmeroError::SignInRequired
    );
    srv.fail_next(&[403, 403]);
    assert_eq!(
        o.list(Listing::Projects, 0, 1, &c).unwrap_err(),
        OmeroError::SignInRequired
    );
    // Refused once: the retry succeeds.
    srv.fail_next(&[403]);
    assert!(o.list(Listing::Projects, 0, 1, &c).is_ok());

    // Signed in and still refused after signing in again: forbidden.
    let srv = server(private());
    let o = connect(&srv, Some(("alice", "s3cret &=?")));
    srv.fail_next(&[403, 403]);
    assert_eq!(
        o.img_data(fake::TILED, &c).unwrap_err(),
        OmeroError::Forbidden
    );
    // A tile failing fails the decode with a readable error.
    let s = session(
        &srv,
        &o,
        descriptor(&srv, fake::TILED, Some(Encoding::Png)),
        None,
    );
    srv.fail_next(&[404]);
    let e = s.decode(&DecodeRequest::full(4), &c).unwrap_err();
    assert!(e.to_string().contains("not found"), "{e}");
    // Not an OMERO server at all.
    let other = specular_remote::testing::TestServer::start(ServerOptions::default(), |_| {
        specular_remote::testing::TestResponse::ok("<html></html>")
    })
    .unwrap();
    let o = Omero::new(client(), &other.url("")).unwrap();
    assert!(matches!(
        o.connect(None, &c).unwrap_err(),
        OmeroError::Malformed(_)
    ));
}

#[test]
fn descriptors_open_through_the_decoder_registry() {
    let srv = server(FakeConfig::synthetic(0));
    specular_omero::registry::set_client(client());
    specular_omero::registry::install();
    let d = descriptor(&srv, fake::STACK, None);
    let dir = temp_dir("registry");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(d.file_name());
    std::fs::write(&path, d.to_json()).unwrap();
    let src = Arc::new(Source::open(&path).unwrap());
    let s = specular_decode::Registry::new().open(src).unwrap();
    assert_eq!(s.info().format, Format::Omero);
    assert_eq!((s.info().width, s.info().height), (64, 48));
    assert!(specular_omero::registry::get(&d.server).is_some());
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert_eq!((r.width, r.height), (64, 48));
    // A descriptor for a server that refuses: the application is asked to sign in.
    let private = server(private());
    let d = descriptor(&private, fake::STACK, None);
    let src = Arc::new(Source::from_vec(d.file_name(), d.to_json()));
    assert!(specular_decode::Registry::new().open(src).is_err());
    assert_eq!(
        specular_omero::registry::take_sign_in_request().as_deref(),
        Some(d.server.as_str())
    );
    assert_eq!(descriptor::safe_name("a/b"), "a_b");
    std::fs::remove_dir_all(&dir).ok();
}

/// Answers saved from IDR (2026-10-12) read as the fake's do.
#[test]
fn idr_answers_parse() {
    let read = |n: &str| -> serde_json::Value {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(n);
        serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()
    };
    let p =
        specular_omero::browse::parse_page(&read("idr-projects.json"), Kind::Project, 0).unwrap();
    assert_eq!((p.items.len(), p.total), (1, 147));
    assert_eq!(p.items[0].id, 101);
    assert_eq!(p.items[0].owner.as_deref(), Some("Public data"));
    assert_eq!(p.items[0].group.as_deref(), Some("Public"));
    let s = specular_omero::browse::parse_page(&read("idr-screens.json"), Kind::Screen, 0).unwrap();
    assert!(s.total > 1 && s.more());
    assert!(s.items[0].name.starts_with("idr"));
    let api = read("idr-api.json");
    assert_eq!(api["data"][0]["version"], "0");
}

/// The Image Data Resource, live: `SPECULAR_OMERO_LIVE=1 cargo test -p specular-omero live`.
#[test]
fn live_idr() {
    if std::env::var("SPECULAR_OMERO_LIVE").as_deref() != Ok("1") {
        return;
    }
    let client = Client::new(ClientConfig::default()).unwrap();
    let o = Omero::new(client, specular_omero::IDR_URL).unwrap();
    let c = CancelToken::new();
    o.connect(None, &c).unwrap();
    let projects = o.list(Listing::Projects, 0, 5, &c).unwrap();
    eprintln!(
        "IDR projects: {} (first {:?})",
        projects.total,
        projects.items.first().map(|p| &p.name)
    );
    assert!(projects.total > 0);
    let p = &projects.items[0];
    let ds = o
        .list(Listing::Datasets { project: p.id }, 0, 1, &c)
        .unwrap();
    let images = o
        .list(
            Listing::Images {
                dataset: ds.items[0].id,
            },
            0,
            1,
            &c,
        )
        .unwrap();
    let id = images.items[0].id;
    let d = o.img_data(id, &c).unwrap();
    eprintln!(
        "IDR image {id}: {:?} tiled {} levels {}",
        d.size, d.tiled, d.levels
    );
    let t = tiles::thumbnail(&o, None, id, 96, &c).unwrap();
    assert!(t.width > 0);
    let desc = Descriptor {
        server: specular_omero::IDR_URL.into(),
        image: id,
        name: d.name.clone(),
        ..Descriptor::default()
    };
    let src = Arc::new(Source::from_vec(desc.file_name(), desc.to_json()));
    let s = open_with(o.clone(), None, src, desc, &c).unwrap();
    let coarsest = *s.info().scales.last().unwrap();
    let r = s.decode(&DecodeRequest::full(coarsest), &c).unwrap();
    eprintln!("IDR decode at 1/{coarsest}: {}×{}", r.width, r.height);
    let screens = o.list(Listing::Screens, 0, 1, &c).unwrap();
    let plates = o
        .list(
            Listing::Plates {
                screen: screens.items[0].id,
            },
            0,
            1,
            &c,
        )
        .unwrap();
    let wells = o
        .list(
            Listing::Wells {
                plate: plates.items[0].id,
            },
            0,
            3,
            &c,
        )
        .unwrap();
    eprintln!(
        "IDR wells: {} ({:?})",
        wells.total,
        wells.items.iter().map(|w| &w.name).collect::<Vec<_>>()
    );
}
