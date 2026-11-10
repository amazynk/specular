//! Screenshot tests of an EPUB page with the contents panel: a page of the synthetic EPUB 3
//! fixture of specular-epub (headings, emphasis, a list and a table) laid out and rendered
//! by the decoder, shown by the GPU image renderer with the table of contents beside it, at
//! 1× and 1.5×, light and dark, and right to left in Hebrew; and its cover page. Rendered
//! offscreen on lavapipe and compared with the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular contents::shots`, then review and
//! commit the rewritten PNGs.

use specular_core::{CancelToken, Orientation, Source};
use specular_decode::{DecodeRequest, Registry, Session};
use specular_render::{ImageRenderer, PxRect, RenderOptions, View};
use specular_shots::{Goldens, Harness};
use specular_theme::Mode;

use super::*;

const SIZE: (f32, f32) = (640.0, 420.0);

fn book() -> Arc<dyn Session> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/formats/specular-epub/fixtures/reflow3.epub"
    );
    Registry::default()
        .open(Arc::new(Source::open(path).unwrap()))
        .unwrap()
}

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    page: u32,
    scale: f32,
    dark: bool,
    rtl: bool,
    panel: bool,
}

fn shoot(h: &mut Harness, goldens: &mut Goldens, s: &Arc<dyn Session>, c: &Case) {
    let pal = Palette::for_mode(if c.dark { Mode::Dark } else { Mode::Light });
    let page = s.open_page(c.page).unwrap();
    let (w, hh) = (page.info().width, page.info().height);
    // The level the view needs: the page about the window's height.
    let k = ((hh as f32 / (SIZE.1 * c.scale)).floor() as u32).clamp(1, 8);
    let k = [1, 2, 4, 8]
        .into_iter()
        .filter(|&s| s <= k)
        .max()
        .unwrap_or(1);
    let r = page
        .decode(&DecodeRequest::full(k), &CancelToken::new())
        .unwrap();
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, w, hh, None);
    renderer.upload_overview(&h.gpu, &mut img, &r);
    let _ = h
        .gpu
        .device
        .poll(specular_gfx::wgpu::PollType::wait_indefinitely());
    let size_px = (
        (SIZE.0 * c.scale).round() as u32,
        (SIZE.1 * c.scale).round() as u32,
    );
    let mut view = View::new();
    view.set_viewport(PxRect {
        x: 0.0,
        y: 0.0,
        w: size_px.0 as f32,
        h: size_px.1 as f32,
    });
    view.upscale_small = true;
    view.set_image(w, hh, Orientation::NORMAL);
    let entries = s.contents().unwrap();
    let panel = ContentsPanel {
        current: current_entry(&entries, c.page),
        entries,
        scroll: 0,
    };
    let (rtl, show) = (c.rtl, c.panel);
    let mut drawn = None;
    let shot = h.render_gpu(
        SIZE,
        c.scale,
        |gpu, enc, target, size| {
            let opts = RenderOptions::default();
            renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
        },
        |dl, text| {
            if show {
                drawn = Some(draw_panel(
                    dl,
                    text,
                    &pal,
                    &panel,
                    (0.0, 0.0, SIZE.0, SIZE.1),
                    0.0,
                    rtl,
                    1.0,
                ));
            }
        },
    );
    goldens.check(&format!("{}-{}x", c.name, c.scale), &shot);
    if let Some((r, rows, _)) = drawn {
        super::super::a11y_audit::audit_screen(c.name, SIZE, rtl, |t| {
            a11y(t, &panel, r, &rows, 1.0);
        });
    }
}

/// A comic's contents (ComicInfo bookmarks, one without a page, nested ones): the panel's
/// screen-reader tree, left to right and in Hebrew, scrolled to the current entry.
#[test]
fn comic_contents_pass_the_audit() {
    let Some(mut h) = Harness::new() else { return };
    let entry = |title: &str, depth, page| ContentsEntry {
        title: title.into(),
        depth,
        page,
    };
    let mut entries = vec![entry("Cover", 0, Some(0))];
    for i in 1..=30u32 {
        entries.push(entry(&format!("Chapter {i}"), 0, Some(i * 4)));
        entries.push(entry(&format!("Chapter {i}, splash"), 1, Some(i * 4 + 1)));
    }
    entries.push(entry("Bonus (missing)", 0, None));
    let panel = ContentsPanel {
        current: current_entry(&entries, 41),
        entries: entries.into(),
        scroll: 12,
    };
    for (name, lang, rtl) in [
        ("comic-contents", specular_i18n::langid!("en-US"), false),
        ("comic-contents-he", specular_i18n::langid!("he"), true),
    ] {
        specular_i18n::I18n::with_thread_language(lang, || {
            let pal = Palette::dark();
            let mut drawn = None;
            h.render(
                SIZE,
                1.0,
                |_| {},
                |dl, text| {
                    drawn = Some(draw_panel(
                        dl,
                        text,
                        &pal,
                        &panel,
                        (0.0, 0.0, SIZE.0, SIZE.1),
                        0.0,
                        rtl,
                        1.0,
                    ));
                },
            );
            let (r, rows, _) = drawn.expect("drawn");
            super::super::a11y_audit::audit_screen(name, SIZE, rtl, |t| {
                a11y(t, &panel, r, &rows, 1.0);
            });
        });
    }
}

#[test]
fn book_pages_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-books",
    );
    let s = book();
    // The first page of the first chapter.
    let chapter = s
        .contents()
        .and_then(|c| c.iter().filter_map(|e| e.page).find(|&p| p > 0))
        .unwrap_or(1);
    let base = Case {
        name: "",
        page: chapter,
        scale: 1.0,
        dark: false,
        rtl: false,
        panel: true,
    };
    let cases = [
        Case {
            name: "epub-cover",
            page: 0,
            panel: false,
            ..base
        },
        Case {
            name: "epub-page-contents",
            ..base
        },
        Case {
            name: "epub-page-contents-dark",
            dark: true,
            scale: 1.5,
            ..base
        },
        Case {
            name: "epub-page",
            panel: false,
            scale: 1.5,
            ..base
        },
    ];
    for c in &cases {
        shoot(&mut h, &mut goldens, &s, c);
    }
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        let c = Case {
            name: "epub-page-contents-he",
            rtl: true,
            ..base
        };
        shoot(&mut h, &mut goldens, &s, &c);
    });
    goldens.finish();
}
