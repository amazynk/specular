//! Screenshot tests of the OMERO sheet at 1× and 1.5×: the servers, a server's settings
//! (light, right to left), and a dataset's images with their thumbnails from the synthetic
//! server (dark; Hebrew, light, right to left; text scale 2). Rendered offscreen on lavapipe
//! and compared with the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular omero::shots`, then review and
//! commit the rewritten PNGs.

use super::*;
use specular_omero::fake::{DATASET, FakeConfig, FakeServer, PROJECT};
use specular_remote::testing::ServerOptions;
use specular_remote::{Client, ClientConfig};
use specular_shots::{Goldens, Harness};
use specular_theme::Color;

const SIZE: (f32, f32) = (900.0, 640.0);
const SCALES: [(f32, &str); 2] = [(1.0, "1x"), (1.5, "1.5x")];

fn backdrop(dl: &mut DrawList) {
    dl.rect(
        Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
        0.0,
        Color::rgba(0.12, 0.12, 0.13, 1.0),
    );
    dl.rect(
        Rect::new(SIZE.0 * 0.3, SIZE.1 * 0.2, SIZE.0 * 0.4, SIZE.1 * 0.6),
        SIZE.1 * 0.3,
        Color::rgba(0.45, 0.45, 0.47, 1.0),
    );
}

/// The synthetic server's dataset of images, browsed, with the thumbnails on the GPU.
fn browsed(h: &mut Harness, fake: &FakeServer) -> (ServerList, Dialog, HashMap<u64, Thumb>) {
    // A fixed address on screen: the synthetic server's port changes from run to run.
    let servers = demo_servers("http://127.0.0.1:4080");
    let client = Client::new(ClientConfig::default()).unwrap();
    let omero = Omero::new(client, &fake.base()).unwrap();
    let cancel = CancelToken::new();
    omero.connect(None, &cancel).unwrap();
    let page = omero
        .list(Listing::Images { dataset: DATASET }, 0, 8, &cancel)
        .unwrap();
    let mut thumbs = HashMap::new();
    for n in &page.items {
        let id = n.image_id().unwrap();
        let img = tiles::thumbnail(&omero, None, id, THUMB, &cancel).unwrap();
        let (tex, tid) =
            super::super::studies::upload_rgba(&h.gpu, &mut h.ui, img.width, img.height, &img.data);
        // The textures live as long as the test.
        std::mem::forget(tex);
        thumbs.insert(
            id,
            Thumb::Shown {
                tex: tid,
                w: img.width,
                h: img.height,
            },
        );
    }
    let mut d = Dialog::new();
    d.page = Page::Search;
    d.server = Some(0);
    let source = |listing| Source {
        listing,
        next: 0,
        total: None,
    };
    d.levels = vec![
        Level {
            title: servers.servers[0].label(),
            sources: vec![source(Listing::Projects), source(Listing::Screens)],
            ..Level::default()
        },
        Level {
            title: "Synthetic project".into(),
            kind: Some(Kind::Project),
            sources: vec![source(Listing::Datasets { project: PROJECT })],
            ..Level::default()
        },
        Level {
            title: "Gradients".into(),
            kind: Some(Kind::Dataset),
            context: Context {
                project: Some("Synthetic project".into()),
                dataset: Some("Gradients".into()),
                ..Context::default()
            },
            sources: vec![Source {
                listing: Listing::Images { dataset: DATASET },
                next: page.offset + page.items.len() as u64,
                total: Some(page.total),
            }],
            rows: page.items,
        },
    ];
    d.selected = Some(Row::Node(1));
    (servers, d, thumbs)
}

fn focus_on(d: &mut Dialog, servers: &ServerList, h: Hit) {
    let none = |_: &Node| Thumb::None;
    d.focus = build_view(d, servers, &none, true)
        .focusables()
        .iter()
        .position(|x| *x == h);
}

fn servers_view(servers: &ServerList) -> DialogView {
    let mut d = Dialog::new();
    d.server = Some(1);
    d.selected = Some(Row::Server(1));
    focus_on(&mut d, servers, Hit::Entry(1));
    build_view(&d, servers, &|_| Thumb::None, false)
}

fn edit_view(servers: &ServerList) -> DialogView {
    let s = &servers.servers[2];
    let mut d = Dialog::new();
    d.page = Page::Edit;
    d.edit = Some(Edit {
        id: Some(s.id.clone()),
        name: LineEdit::new(&s.name),
        url: LineEdit::new(&s.url),
        user: LineEdit::new(&s.user),
        password: LineEdit::new("hunter2"),
        has_password: false,
        tried: false,
    });
    d.message = Some((t!("omero-session-ended", server = s.label()), Tone::Warning));
    focus_on(&mut d, servers, Hit::Form(3));
    build_view(&d, servers, &|_| Thumb::None, true)
}

#[test]
fn omero_sheet_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let Ok(fake) = FakeServer::start(FakeConfig::synthetic(12), ServerOptions::default()) else {
        return;
    };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-omero-sheet",
    );
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    let (servers, mut d, thumbs) = browsed(&mut h, &fake);
    focus_on(&mut d, &servers, Hit::Entry(1));
    let thumb = |n: &Node| {
        n.image_id()
            .and_then(|id| thumbs.get(&id).copied())
            .unwrap_or(Thumb::None)
    };
    let views = |hebrew: bool| {
        specular_i18n::I18n::with_thread_language(
            if hebrew { he.clone() } else { en.clone() },
            || {
                (
                    servers_view(&servers),
                    edit_view(&servers),
                    build_view(&d, &servers, &thumb, false),
                )
            },
        )
    };
    let (list, _, browse) = views(false);
    // Right to left in Hebrew: the views made in the language they are drawn in.
    let (_, edit_he, browse_he) = views(true);
    let pal_dark = Palette::dark();
    let pal_light = Palette::light();
    for (scale, suffix) in SCALES {
        for (name, v, pal, rtl, ts) in [
            ("servers", &list, &pal_dark, false, 1.0),
            ("edit-rtl-light", &edit_he, &pal_light, true, 1.0),
            ("browse", &browse, &pal_dark, false, 1.0),
            ("browse-he-light", &browse_he, &pal_light, true, 1.0),
            ("browse-text-2", &browse, &pal_dark, false, 2.0),
        ] {
            let lang = if rtl { &he } else { &en };
            let (shot, tree) = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let mut drawn = None;
                let shot = h.render(SIZE, scale, backdrop, |dl, text| {
                    drawn = Some(view::draw(dl, text, pal, v, SIZE, rtl, ts));
                });
                let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
                view::a11y(&mut tree, v, &drawn.expect("drawn"), 1.0);
                (shot, tree)
            });
            goldens.check(&format!("omero-{name}-{suffix}"), &shot);
            super::super::a11y_audit::expect_clean(&format!("omero-{name}-{suffix}"), &tree, rtl);
        }
    }
    goldens.finish();
}

/// Every control is inside the sheet, nothing overlaps, right to left mirrors, and the
/// thumbnails sit in their rows.
#[test]
fn controls_sit_in_the_sheet_and_mirror() {
    let Some(mut h) = Harness::new() else { return };
    let Ok(fake) = FakeServer::start(FakeConfig::synthetic(12), ServerOptions::default()) else {
        return;
    };
    let (servers, d, thumbs) = browsed(&mut h, &fake);
    assert!(thumbs.len() >= 8, "{} thumbnails", thumbs.len());
    let thumb = |n: &Node| {
        n.image_id()
            .and_then(|id| thumbs.get(&id).copied())
            .unwrap_or(Thumb::None)
    };
    let pal = Palette::dark();
    for v in [
        servers_view(&servers),
        edit_view(&servers),
        build_view(&d, &servers, &thumb, false),
    ] {
        let mut out = Vec::new();
        h.render(SIZE, 1.0, backdrop, |dl, text| {
            out.push(view::draw(dl, text, &pal, &v, SIZE, false, 1.0));
            out.push(view::draw(dl, text, &pal, &v, SIZE, true, 1.0));
            out.push(view::draw(dl, text, &pal, &v, SIZE, false, 2.0));
        });
        let (l, r) = (&out[0], &out[1]);
        assert_eq!(l.sheet, r.sheet);
        for dr in [&out[0], &out[2]] {
            let s = dr.sheet;
            for (i, (a, ha)) in dr.hits.iter().enumerate() {
                assert!(
                    a.x >= s.x - 0.01
                        && a.right() <= s.right() + 0.01
                        && a.y >= s.y - 0.01
                        && a.bottom() <= s.bottom() + 0.01,
                    "{ha:?} {a:?} outside {s:?}"
                );
                for (b, hb) in &dr.hits[i + 1..] {
                    let apart = a.right() <= b.x + 0.01
                        || b.right() <= a.x + 0.01
                        || a.bottom() <= b.y + 0.01
                        || b.bottom() <= a.y + 0.01;
                    assert!(apart, "{ha:?} {a:?} overlaps {hb:?} {b:?}");
                }
            }
        }
        for ((a, ha), (b, hb)) in l.hits.iter().zip(&r.hits) {
            assert_eq!(ha, hb);
            assert!((a.x - (SIZE.0 - b.right())).abs() < 0.01, "{ha:?}");
        }
        for f in v.focusables() {
            if let Hit::Entry(k) = f
                && k >= v.scroll + l.visible
            {
                continue;
            }
            assert!(l.hits.iter().any(|(_, h)| *h == f), "{f:?} not drawn");
        }
    }
}
