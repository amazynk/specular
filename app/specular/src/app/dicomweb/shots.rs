//! Screenshot tests of the DICOMweb sheet at 1× and 1.5×: the servers, a server's settings
//! (light, right to left), search results from the synthetic server (dark; Hebrew, light,
//! right to left; text scale 2), and the same results with patient data shown. Rendered
//! offscreen on lavapipe and compared with the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular dicomweb::shots`, then review and
//! commit the rewritten PNGs.

use super::view::{a11y, a11y_target, draw};
use super::*;
use specular_dicomweb::fake::{FakeConfig, FakeServer};
use specular_remote::testing::ServerOptions;
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

/// The servers the demo shows, and a search on the first (the synthetic server), with the
/// first study's series listed.
fn searched(fake: &FakeServer) -> (ServerList, Dialog) {
    // A fixed address on screen: the synthetic server's port changes from run to run.
    let servers = demo_servers("http://127.0.0.1:8042/dicom-web");
    let client = Client::new(ClientConfig::default()).unwrap();
    let web = DicomWeb::new(client, &fake.base(), Auth::None);
    let mut d = Dialog::new();
    d.page = Page::Search;
    d.server = Some(0);
    d.search.modality.set_text("CT");
    let mut query = d.query();
    query.limit = 6;
    let page = web.search_studies(&query, &CancelToken::new()).unwrap();
    d.search.more = page.more;
    d.search.studies = page.rows;
    d.search.query = Some(query);
    let study = d.search.studies[0].study_uid.clone();
    let series = web
        .search_series(&study, PAGE_SIZE, 0, &CancelToken::new())
        .unwrap();
    d.search.open = Some(0);
    d.search.series = series.rows;
    d.selected = Some(Row::Series(0, 0));
    d.focus = build_view(&d, &servers, false)
        .focusables()
        .iter()
        .position(|h| *h == Hit::Entry(1));
    (servers, d)
}

fn servers_view(servers: &ServerList) -> DialogView {
    let mut d = Dialog::new();
    d.server = Some(1);
    d.selected = Some(Row::Server(1));
    d.focus = Some(1);
    build_view(&d, servers, false)
}

/// Remove's question about the selected server.
fn remove_view(servers: &ServerList) -> DialogView {
    let mut d = Dialog::new();
    d.server = Some(1);
    d.selected = Some(Row::Server(1));
    d.removing = Some(servers.servers[1].id.clone());
    let v = build_view(&d, servers, false);
    d.focus = v
        .focusables()
        .iter()
        .position(|h| *h == Hit::Button(Button::Cancel));
    build_view(&d, servers, false)
}

/// The servers with a DIMSE one added (selected by the Send views).
fn with_dimse(servers: &ServerList) -> ServerList {
    let mut l = servers.clone();
    l.upsert(Server {
        name: "Hospital PACS".into(),
        kind: ServerKind::Dimse,
        host: "pacs.example.org".into(),
        port: 11112,
        ae_title: "ARCHIVE".into(),
        ..Server::default()
    });
    l
}

/// The Send page at `stage`: 0 choose, 1 confirm, 2 running, 3 done with failures.
fn send_view(servers: &ServerList, stage: u8) -> DialogView {
    use send::{Choice, Failed, Scope, SendRequest, Sending, Stage};
    let files = |n: usize| (0..n).map(|i| PathBuf::from(format!("IM{i:05}"))).collect();
    let mut d = Dialog::new();
    d.page = Page::Send;
    let last = servers.servers.len() - 1;
    d.server = Some(last);
    d.selected = Some(Row::Server(last));
    let mut s = Sending::new(SendRequest {
        choices: vec![
            Choice {
                scope: Scope::Series,
                files: files(24),
                studies: 1,
            },
            Choice {
                scope: Scope::Study,
                files: files(48),
                studies: 1,
            },
            Choice {
                scope: Scope::All,
                files: files(120),
                studies: 3,
            },
        ],
        at: 1,
    });
    s.server = "Hospital PACS".into();
    s.host = "pacs.example.org:11112".into();
    match stage {
        1 => s.stage = Stage::Confirm,
        2 => {
            s.stage = Stage::Running {
                rx: crossbeam_channel::never(),
                cancel: CancelToken::new(),
            };
            s.done = 17;
        }
        3 => {
            s.stage = Stage::Done;
            s.done = 48;
            s.failed = vec![
                Failed {
                    name: "IM00007".into(),
                    reason: t!("pacs-reason-burned-in"),
                },
                Failed {
                    name: "IM00031".into(),
                    reason: send::status_text(0xA700),
                },
            ];
        }
        _ => {}
    }
    d.send = Some(s);
    d.focus = Some(0);
    build_view(&d, servers, false)
}

/// A DIMSE server's form: a bad AE title tried, or (`mv`) set to C-MOVE.
fn edit_dimse_view(servers: &ServerList, mv: bool) -> DialogView {
    let mut d = Dialog::new();
    d.page = Page::Edit;
    d.edit = Some(Edit {
        id: Some("x".into()),
        name: LineEdit::new("Hospital PACS"),
        url: LineEdit::new(""),
        auth: AuthKind::None,
        user: LineEdit::new(""),
        secret: LineEdit::new(""),
        has_secret: false,
        tried: true,
        kind: ServerKind::Dimse,
        host: LineEdit::new("pacs.example.org"),
        port: LineEdit::new("11112"),
        ae: LineEdit::new(if mv { "ARCHIVE" } else { "ARCHIVE\\X" }),
        our_ae: LineEdit::new(""),
        tls: !mv,
        retrieve: if mv { Retrieve::Move } else { Retrieve::Get },
        receive_address: LineEdit::new(if mv { "192.168.1.20" } else { "" }),
        receive_port: LineEdit::new(if mv { "11113" } else { "" }),
    });
    d.focus = Some(if mv { 7 } else { 4 });
    build_view(&d, servers, false)
}

fn edit_view(servers: &ServerList) -> DialogView {
    let s = &servers.servers[1];
    let mut d = Dialog::new();
    d.page = Page::Edit;
    d.edit = Some(Edit {
        id: Some(s.id.clone()),
        name: LineEdit::new(&s.name),
        url: LineEdit::new("http://pacs.example.org/dicom-web"),
        auth: s.auth,
        user: LineEdit::new(&s.user),
        secret: LineEdit::new("hunter2"),
        has_secret: false,
        tried: false,
        kind: specular_dicomweb::ServerKind::DicomWeb,
        host: LineEdit::new(""),
        port: LineEdit::new(""),
        ae: LineEdit::new(""),
        our_ae: LineEdit::new(""),
        tls: false,
        retrieve: Retrieve::Get,
        receive_address: LineEdit::new(""),
        receive_port: LineEdit::new(""),
    });
    d.message = Some((t!("dicomweb-secret-needed"), Tone::Warning));
    d.focus = Some(4);
    build_view(&d, servers, false)
}

#[test]
fn dicomweb_sheet_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let Ok(fake) = FakeServer::start(FakeConfig::synthetic(8, 24, 64), ServerOptions::default())
    else {
        return;
    };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-dicomweb-sheet",
    );
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    let (servers, mut d) = searched(&fake);
    let views = |hebrew: bool, d: &Dialog| {
        specular_i18n::I18n::with_thread_language(
            if hebrew { he.clone() } else { en.clone() },
            || {
                (
                    servers_view(&servers),
                    edit_view(&servers),
                    build_view(d, &servers, false),
                )
            },
        )
    };
    let (list, _, search) = views(false, &d);
    let pacs = with_dimse(&servers);
    let sends: Vec<DialogView> = (0..4).map(|k| send_view(&pacs, k)).collect();
    let edit_dimse = edit_dimse_view(&pacs, false);
    let edit_dimse_he =
        specular_i18n::I18n::with_thread_language(he.clone(), || edit_dimse_view(&pacs, false));
    let edit_move = edit_dimse_view(&pacs, true);
    let edit_move_he =
        specular_i18n::I18n::with_thread_language(he.clone(), || edit_dimse_view(&pacs, true));
    let send_he = specular_i18n::I18n::with_thread_language(he.clone(), || send_view(&pacs, 3));
    let remove = remove_view(&servers);
    let remove_he = specular_i18n::I18n::with_thread_language(he.clone(), || remove_view(&servers));
    // Right to left in Hebrew: the views made in the language they are drawn in.
    let (_, edit_he, search_he) = views(true, &d);
    d.show_patient = true;
    let (_, _, patient) = views(false, &d);
    let pal_dark = Palette::dark();
    let pal_light = Palette::light();
    for (scale, suffix) in SCALES {
        for (name, v, pal, rtl, ts) in [
            ("servers", &list, &pal_dark, false, 1.0),
            ("edit-rtl-light", &edit_he, &pal_light, true, 1.0),
            ("search", &search, &pal_dark, false, 1.0),
            ("search-he-light", &search_he, &pal_light, true, 1.0),
            ("search-text-2", &search, &pal_dark, false, 2.0),
            ("patient-shown", &patient, &pal_dark, false, 1.0),
            ("send", &sends[0], &pal_dark, false, 1.0),
            ("send-confirm", &sends[1], &pal_dark, false, 1.0),
            ("send-running", &sends[2], &pal_light, false, 1.0),
            ("send-done", &sends[3], &pal_dark, false, 1.0),
            ("send-done-he-light", &send_he, &pal_light, true, 1.0),
            ("edit-dimse", &edit_dimse, &pal_dark, false, 1.0),
            ("edit-dimse-he-light", &edit_dimse_he, &pal_light, true, 1.0),
            ("edit-dimse-move", &edit_move, &pal_dark, false, 1.0),
            (
                "edit-dimse-move-he-light",
                &edit_move_he,
                &pal_light,
                true,
                1.0,
            ),
            ("remove-ask", &remove, &pal_dark, false, 1.0),
            ("remove-ask-he-light", &remove_he, &pal_light, true, 1.0),
        ] {
            let lang = if rtl { &he } else { &en };
            let (shot, tree) = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let mut drawn = None;
                let shot = h.render(SIZE, scale, backdrop, |dl, text| {
                    drawn = Some(draw(dl, text, pal, v, SIZE, rtl, ts));
                });
                let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
                a11y(&mut tree, v, &drawn.expect("drawn"), 1.0);
                (shot, tree)
            });
            goldens.check(&format!("dicomweb-{name}-{suffix}"), &shot);
            super::super::a11y_audit::expect_clean(
                &format!("dicomweb-{name}-{suffix}"),
                &tree,
                rtl,
            );
        }
    }
    goldens.finish();
}

/// Every control is inside the sheet, nothing overlaps, right-to-left mirrors, large text
/// still fits, and patient data reaches neither the screen nor the screen reader while hidden.
#[test]
fn controls_sit_in_the_sheet_and_mirror() {
    let Some(mut h) = Harness::new() else { return };
    let Ok(fake) = FakeServer::start(FakeConfig::synthetic(8, 24, 64), ServerOptions::default())
    else {
        return;
    };
    let (servers, d) = searched(&fake);
    let pal = Palette::dark();
    for v in [
        servers_view(&servers),
        remove_view(&servers),
        edit_view(&servers),
        build_view(&d, &servers, false),
    ] {
        let mut out = Vec::new();
        h.render(SIZE, 1.0, backdrop, |dl, text| {
            out.push(draw(dl, text, &pal, &v, SIZE, false, 1.0));
            out.push(draw(dl, text, &pal, &v, SIZE, true, 1.0));
            out.push(draw(dl, text, &pal, &v, SIZE, false, 2.0));
        });
        let (l, r) = (&out[0], &out[1]);
        assert_eq!(l.sheet, r.sheet);
        for d in [&out[0], &out[2]] {
            let s = d.sheet;
            for (i, (a, ha)) in d.hits.iter().enumerate() {
                assert!(
                    a.x >= s.x - 0.01
                        && a.right() <= s.right() + 0.01
                        && a.y >= s.y - 0.01
                        && a.bottom() <= s.bottom() + 0.01,
                    "{ha:?} {a:?} outside {s:?}"
                );
                for (b, hb) in &d.hits[i + 1..] {
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
        // Every focusable control can be clicked, and maps back from its a11y node.
        let mut tree = accesskit::TreeUpdate {
            nodes: vec![(
                accesskit::NodeId(1),
                accesskit::Node::new(accesskit::Role::Window),
            )],
            tree: None,
            tree_id: accesskit::TreeId::ROOT,
            focus: accesskit::NodeId(1),
        };
        a11y(&mut tree, &v, l, 1.0);
        for f in v.focusables() {
            if let Hit::Entry(k) = f
                && k >= v.scroll + l.visible
            {
                continue;
            }
            assert!(l.hits.iter().any(|(_, h)| *h == f), "{f:?} not drawn");
        }
        for (id, _) in &tree.nodes {
            if let Some(h) = a11y_target(&v, id.0) {
                assert!(v.focusables().contains(&h) || matches!(h, Hit::Button(_)));
            }
        }
        if v.page == Page::Search {
            let text: String = tree
                .nodes
                .iter()
                .map(|(_, n)| format!("{:?} {:?} ", n.label(), n.value()))
                .collect();
            let row = &d.search.studies[0];
            assert!(
                !text.contains(&row.patient.id),
                "patient ID in the a11y tree"
            );
            assert!(!text.contains(&row.accession), "accession in the a11y tree");
        }
    }
}
