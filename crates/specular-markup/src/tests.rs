use std::path::PathBuf;
use std::sync::Arc;

use crate::align::{Grey, Similarity, auto};
use crate::compare::{View, changes, compose, warp};
use crate::*;

fn sample(author: &str) -> Vec<Markup> {
    let mk = |shape, text: &str| {
        let mut m = Markup::new(shape, [229, 57, 53], 1.5, 12.0, author);
        m.text = text.into();
        m
    };
    vec![
        mk(
            Shape::Cloud(vec![
                P::new(100.0, 100.0),
                P::new(300.0, 100.0),
                P::new(300.0, 200.0),
                P::new(100.0, 200.0),
            ]),
            "Revise",
        ),
        mk(
            Shape::Text(P::new(50.0, 300.0)),
            "Check beam size\nsee detail 4",
        ),
        mk(
            Shape::Arrow {
                tail: P::new(400.0, 400.0),
                head: P::new(500.0, 350.0),
            },
            "Leak",
        ),
        mk(
            Shape::Stamp {
                at: P::new(600.0, 100.0),
                kind: Stamp::Approved,
            },
            "",
        ),
        mk(
            Shape::Stamp {
                at: P::new(600.0, 200.0),
                kind: Stamp::Custom,
            },
            "Für Bau",
        ),
        mk(
            Shape::Length(P::new(100.0, 500.0), P::new(383.5, 500.0)),
            "5.000 m",
        ),
        mk(
            Shape::Area(vec![
                P::new(400.0, 450.0),
                P::new(500.0, 450.0),
                P::new(450.0, 520.0),
            ]),
            "3.5 m²",
        ),
        mk(
            Shape::Pin {
                at: P::new(700.0, 500.0),
                photo: PathBuf::from("/site/2026-10-24/IMG_0042 (1).jpg"),
            },
            "IMG_0042 (1).jpg",
        ),
    ]
}

fn words(m: &Markup) -> String {
    match &m.shape {
        Shape::Stamp {
            kind: Stamp::Custom,
            ..
        } => m.text.to_uppercase(),
        Shape::Stamp { kind, .. } => kind.id().to_uppercase(),
        _ => String::new(),
    }
}

fn close(a: &[Markup], b: &[Markup], tol: f64) {
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(b) {
        assert_eq!(x.id, y.id);
        assert_eq!(x.text, y.text);
        assert_eq!(x.author, y.author);
        assert_eq!(x.colour, y.colour);
        assert_eq!(x.shape.kind(), y.shape.kind());
        assert_eq!(&x.date[..16], &y.date[..16]);
        assert!((x.width - y.width).abs() < tol, "{} {}", x.width, y.width);
        assert!((x.size - y.size).abs() < tol);
        for (p, q) in x.shape.points().iter().zip(y.shape.points()) {
            assert!(p.dist(q) < tol, "{p:?} {q:?}");
        }
        match (&x.shape, &y.shape) {
            (Shape::Stamp { kind: a, .. }, Shape::Stamp { kind: b, .. }) => assert_eq!(a, b),
            (Shape::Pin { photo: a, .. }, Shape::Pin { photo: b, .. }) => assert_eq!(a, b),
            _ => {}
        }
    }
}

#[test]
fn xfdf_round_trip_on_a_rotated_page() {
    let items = sample("Marc");
    // A page shown turned a quarter (/Rotate 90) of 842 × 595 pt: user (x, y) → (y, x).
    let rot: Affine = [0.0, 1.0, 1.0, 0.0, 0.0, 0.0];
    let plain: Affine = [1.0, 0.0, 0.0, -1.0, 0.0, 595.0];
    let maps = |p: u32| match p {
        0 => Some(rot),
        2 => Some(plain),
        _ => None,
    };
    let text = xfdf::write(
        "plan & co.pdf",
        &[(0, &items), (2, &items[..3])],
        &maps,
        &words,
    );
    assert!(text.contains("<f href=\"plan &amp; co.pdf\"/>"));
    assert!(text.contains("style=\"cloudy\""));
    assert!(text.contains("intent=\"LineDimension\""));
    assert!(text.contains("icon=\"Approved\""));
    let back = xfdf::read(&text, &maps).unwrap();
    assert_eq!(back.len(), 2);
    close(&items, &back[&0], 1e-3);
    close(&items[..3], &back[&2], 1e-3);
    // Pages the PDF does not have are dropped; garbage is an error.
    assert!(xfdf::read(&text, &|_| None).unwrap().is_empty());
    assert!(xfdf::read("<html/>", &maps).is_err());
    assert!(xfdf::read("not xml", &maps).is_err());
}

#[test]
fn xfdf_user_space_is_the_pdf_s() {
    let items = sample("A");
    let plain: Affine = [1.0, 0.0, 0.0, -1.0, 0.0, 600.0];
    let text = xfdf::write("p.pdf", &[(0, &items[5..6])], &|_| Some(plain), &words);
    // The length from (100, 500) to (383.5, 500) on the page, y down, is at y = 100 in user
    // space.
    assert!(
        text.contains("start=\"100,100\" end=\"383.5,100\""),
        "{text}"
    );
}

#[test]
fn json_round_trip_and_rescale() {
    let items = sample("Dana");
    let text = json::write(
        "plan \"A\".dxf",
        (1000, 800),
        &[(0, &items), (3, &items[..1])],
    );
    let back = json::read(&text, (1000, 800)).unwrap();
    close(&items, &back[&0], 1e-3);
    close(&items[..1], &back[&3], 1e-3);
    // Shown at twice the size later: the markup follows.
    let big = json::read(&text, (2000, 1600)).unwrap();
    let p = big[&0][0].shape.points()[0];
    assert!(p.dist(P::new(200.0, 200.0)) < 1e-9);
    assert!((big[&0][0].width - 3.0).abs() < 1e-9);
    assert!(json::read("{}", (1, 1)).is_err());
}

#[test]
fn undo_and_redo() {
    let mut d = Doc::new(Space::Pixels);
    let items = sample("x");
    assert!(!d.can_undo());
    assert!(d.edit(0, |v| v.push(items[0].clone())));
    assert!(d.edit(1, |v| v.push(items[1].clone())));
    assert!(d.edit(0, |v| v[0].shape.map(|p| P::new(p.x + 10.0, p.y))));
    // No change: no step.
    assert!(!d.edit(0, |_| {}));
    assert_eq!(d.len(), 2);
    assert_eq!(d.undo(), Some(0));
    assert_eq!(d.page(0)[0], items[0]);
    assert_eq!(d.undo(), Some(1));
    assert!(d.page(1).is_empty());
    assert_eq!(d.pages().count(), 1);
    assert_eq!(d.redo(), Some(1));
    assert_eq!(d.page(1)[0], items[1]);
    // A new edit drops what could be redone.
    assert!(d.edit(0, |v| v.clear()));
    assert!(!d.can_redo());
    assert!(d.page(0).is_empty());
    assert_eq!(d.undo(), Some(0));
    assert_eq!(d.page(0).len(), 1);
    assert_eq!(d.undo(), Some(1));
    assert_eq!(d.undo(), Some(0));
    assert!(d.is_empty());
    assert_eq!(d.undo(), None);
}

#[test]
fn clouds_bulge_outwards_either_way_round() {
    let sq = vec![
        P::new(0.0, 0.0),
        P::new(100.0, 0.0),
        P::new(100.0, 50.0),
        P::new(0.0, 50.0),
    ];
    let mut rev = sq.clone();
    rev.reverse();
    for pts in [sq, rev] {
        let arcs = cloud::arcs(&pts, 10.0);
        assert_eq!(arcs.len(), 30);
        for a in &arcs {
            let mid = a.at(a.start + a.sweep / 2.0);
            let inside =
                mid.x > 0.0 + 1e-6 && mid.x < 100.0 - 1e-6 && mid.y > 1e-6 && mid.y < 50.0 - 1e-6;
            assert!(!inside, "{mid:?}");
        }
        let line = cloud::flatten(&arcs, 0.1);
        assert!(line.len() > 60);
        let bez = cloud::beziers(&arcs);
        // Each half circle is two quarter turns; ends meet the next arc.
        assert_eq!(bez.len(), 60);
        for w in bez.windows(2) {
            assert!(w[0][3].dist(w[1][0]) < 1e-9);
        }
    }
}

#[test]
fn dates_and_colours() {
    assert_eq!(iso_of(0), "1970-01-01T00:00:00Z");
    assert_eq!(iso_of(1_792_850_580), "2026-10-24T14:03:00Z");
    assert_eq!(pdf_date("2026-10-24T14:03:00Z"), "D:20261024140300Z");
    assert_eq!(
        iso_of_pdf_date("D:20261024140300+02'00'"),
        "2026-10-24T14:03:00+02:00"
    );
    assert_eq!(iso_of_pdf_date("D:2026"), "2026-01-01T00:00:00Z");
    assert_eq!(day_of("2026-10-24T14:03:00Z"), "2026-10-24");
    // Local time keeps its offset there and back: a stamp placed at 01:30 in Sydney shows
    // that day, not the day before in UTC.
    let syd = "2026-10-25T01:30:00+11:00";
    assert_eq!(pdf_date(syd), "D:20261025013000+11'00'");
    assert_eq!(iso_of_pdf_date(&pdf_date(syd)), syd);
    assert_eq!(
        pdf_date("2026-10-24T09:00:00-05:30"),
        "D:20261024090000-05'30'"
    );
    assert_eq!(day_of(syd), "2026-10-25");
    let now = now_local();
    assert_eq!(now.len(), 25, "{now}");
    assert_eq!(iso_of_pdf_date(&pdf_date(&now)), now);
    let m = Markup::new(Shape::Text(P::new(0.0, 0.0)), [0, 0, 0], 1.0, 12.0, "x");
    assert_eq!(m.date.len(), 25, "placed in local time: {}", m.date);
    assert_eq!(
        parse_colour(&hex_colour([1, 128, 255])),
        Some([1, 128, 255])
    );
    assert_eq!(parse_colour("#f00"), Some([255, 0, 0]));
    assert_ne!(new_id(), new_id());
    assert_eq!(
        sidecar_path(std::path::Path::new("/d/plan.pdf"), Space::PdfPage),
        PathBuf::from("/d/plan.pdf.markup.xfdf")
    );
}

/// A drawing page (842 × 595 pt) with a frame and a few lines.
fn sheet_pdf(rotate: u32) -> Vec<u8> {
    let content = "0 0 0 RG 2 w 20 20 802 555 re S 100 100 m 700 100 l S 100 100 m 100 500 l S";
    let objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 842 595] /Rotate {rotate} /Contents 4 0 R >>"
        ),
        specular_pdf::fixture::stream("", content),
    ];
    specular_pdf::fixture::raw(&objects)
}

/// The page rendered at 1 px per point, RGBA.
fn render(pdf: &[u8]) -> (u32, u32, Vec<u8>) {
    let doc = specular_pdf::Document::open(Arc::new(pdf.to_vec())).unwrap();
    let (w, h) = doc.page_size(0).unwrap();
    let (w, h) = (w.round() as u32, h.round() as u32);
    let req = specular_pdf::RenderRequest {
        page: 0,
        scale_x: 1.0,
        scale_y: 1.0,
        x: 0,
        y: 0,
        width: w,
        height: h,
    };
    (w, h, doc.render(req, &|| false).unwrap())
}

fn reddish(px: &[u8], w: u32, x: f64, y: f64, r: i32) -> bool {
    let mut n = 0;
    for dy in -r..=r {
        for dx in -r..=r {
            let (xx, yy) = (x as i32 + dx, y as i32 + dy);
            if xx < 0 || yy < 0 {
                continue;
            }
            let i = (yy as usize * w as usize + xx as usize) * 4;
            if let Some(p) = px.get(i..i + 3)
                && p[0] > 150
                && i32::from(p[0]) - i32::from(p[1]) > 60
                && i32::from(p[0]) - i32::from(p[2]) > 60
            {
                n += 1;
            }
        }
    }
    n > 0
}

#[test]
fn exported_pdf_shows_the_markup_in_hayro() {
    for rotate in [0, 90] {
        let src = sheet_pdf(rotate);
        let (w, h, before) = render(&src);
        let mut d = Doc::new(Space::PdfPage);
        let mut items = sample("Marc");
        if rotate == 90 {
            // The page shows 595 pt wide.
            for m in &mut items {
                m.shape.map(|p| P::new(p.x * 0.7, p.y));
            }
        }
        d.edit(0, |v| *v = items.clone());
        let out = pdf::export(&src, &d, 1.0, &words, None).unwrap();
        assert!(out.starts_with(&src), "the original bytes are kept");
        assert_eq!(
            specular_pdf::annotate::annotation_count(&out, 0),
            Some(items.len())
        );
        let (w2, h2, after) = render(&out);
        assert_eq!((w, h), (w2, h2), "the page keeps its size");
        // Displayed coordinates are the rendered pixels at 1 px/pt.
        for m in &items {
            let probe = match &m.shape {
                Shape::Cloud(p) => P::new((p[0].x + p[1].x) / 2.0, p[0].y - 4.0),
                Shape::Text(at) => P::new(at.x + 4.0, at.y + 8.0),
                Shape::Arrow { head, .. } => P::new(head.x - 3.0, head.y + 1.5),
                Shape::Stamp { at, .. } => {
                    let (fw, _) = stamp_extent(&words(m), &m.stamp_line(), m.size);
                    P::new(at.x - fw / 2.0, at.y)
                }
                Shape::Length(a, b) => P::new((a.x + b.x) / 2.0 + 30.0, a.y),
                Shape::Area(p) => P::new((p[0].x + p[1].x) / 2.0, p[0].y),
                Shape::Pin { at, .. } => pin_marker(*at, m.size).0,
            };
            assert!(
                reddish(&after, w, probe.x, probe.y, 2),
                "rotate {rotate}: {} not drawn at {probe:?}",
                m.shape.kind()
            );
            assert!(!reddish(&before, w, probe.x, probe.y, 2));
        }
        // The sheet's own lines are still there, black.
        let i = ((495 * w + 400) * 4) as usize;
        if rotate == 0 {
            assert!(after[i] < 80 && before[i] < 80, "{:?}", &after[i..i + 4]);
        }
    }
}

#[test]
fn export_refuses_encrypted_files() {
    let mut d = Doc::new(Space::PdfPage);
    d.edit(0, |v| v.push(sample("x").remove(0)));
    let r = pdf::export(&specular_pdf::fixture::encrypted(), &d, 1.0, &words, None);
    assert!(r.is_err());
}

/// A synthetic drawing: a frame, a grid of rooms, a title block and scattered marks.
fn synthetic_sheet(w: usize, h: usize) -> Grey {
    let mut px = vec![255u8; w * h];
    let mut line = |x0: usize, y0: usize, x1: usize, y1: usize, t: usize| {
        for y in y0.min(y1)..=y0.max(y1) + t {
            for x in x0.min(x1)..=x0.max(x1) + t {
                if x < w && y < h {
                    px[y * w + x] = 0;
                }
            }
        }
    };
    line(30, 30, w - 34, 30, 3);
    line(30, h - 34, w - 34, h - 34, 3);
    line(30, 30, 30, h - 34, 3);
    line(w - 34, 30, w - 34, h - 34, 3);
    let mut seed = 12345u64;
    let mut rnd = |m: usize| {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 33) as usize) % m
    };
    for _ in 0..60 {
        let (x, y) = (60 + rnd(w - 300), 60 + rnd(h - 300));
        let (a, b) = (20 + rnd(200), 20 + rnd(160));
        line(x, y, x + a, y, 1);
        line(x, y, x, y + b, 1);
        line(x + a, y, x + a, y + b, 1);
        line(x, y + b, x + a, y + b, 1);
    }
    for _ in 0..300 {
        let (x, y) = (60 + rnd(w - 120), 60 + rnd(h - 120));
        line(x, y, x + 4 + rnd(10), y, 0);
        line(x, y, x, y + 3 + rnd(8), 0);
    }
    Grey::new(w, h, px)
}

/// `g` drawn through `t` onto a `w × h` sheet (as a re-plotted or re-scanned revision).
fn transformed(g: &Grey, t: &Similarity, w: usize, h: usize) -> Grey {
    warp(g, t, w, h)
}

fn worst_error(found: &Similarity, truth: &Similarity, w: usize, h: usize) -> f64 {
    let mut worst = 0f64;
    for p in [
        P::new(0.0, 0.0),
        P::new(w as f64, 0.0),
        P::new(0.0, h as f64),
        P::new(w as f64, h as f64),
        P::new(w as f64 / 2.0, h as f64 / 2.0),
    ] {
        worst = worst.max(found.apply(p).dist(truth.apply(p)));
    }
    worst
}

#[test]
fn automatic_alignment_is_sub_pixel() {
    let a = synthetic_sheet(1800, 1300);
    // The other sheet: a's content turned 1.2°, scaled by 1.04 and shifted (a re-scan).
    let a_to_b = Similarity::new(1.04, 1.2f64.to_radians(), 37.3, -21.6);
    let b = transformed(&a, &a_to_b, 1900, 1400);
    let truth = a_to_b.inverse();
    let t0 = std::time::Instant::now();
    let got = auto(&a, &b, None).expect("aligned");
    let worst = worst_error(&got.t, &truth, b.w, b.h);
    eprintln!(
        "auto alignment: worst {worst:.3} px, rms {:.3}, {} matches, {:?}",
        got.rms,
        got.matches,
        t0.elapsed()
    );
    assert!(worst < 1.0, "worst error {worst} px");
    assert!(got.matches >= 6);
}

#[test]
fn two_point_pairs_then_refined() {
    let a = synthetic_sheet(1600, 1100);
    let a_to_b = Similarity::new(0.97, -0.6f64.to_radians(), -15.0, 40.0);
    let b = transformed(&a, &a_to_b, 1600, 1100);
    let truth = a_to_b.inverse();
    // Exact pairs give the exact map.
    let (a1, a2) = (P::new(100.0, 200.0), P::new(1400.0, 900.0));
    let (b1, b2) = (a_to_b.apply(a1), a_to_b.apply(a2));
    let t = Similarity::from_pairs(a1, a2, b1, b2).unwrap();
    assert!(worst_error(&t, &truth, 1600, 1100) < 1e-6);
    // Picks off by a few pixels are refined to sub-pixel.
    let rough = Similarity::from_pairs(
        a1,
        a2,
        P::new(b1.x + 3.0, b1.y - 2.0),
        P::new(b2.x - 2.0, b2.y + 3.0),
    )
    .unwrap();
    assert!(worst_error(&rough, &truth, 1600, 1100) > 2.0);
    let got = auto(&a, &b, Some(rough)).unwrap();
    let worst = worst_error(&got.t, &truth, 1600, 1100);
    eprintln!(
        "refined from picks: worst {worst:.3} px, rms {:.3}",
        got.rms
    );
    assert!(worst < 1.0, "{worst}");
}

#[test]
fn similarity_algebra() {
    let t = Similarity::new(2.0, 0.3, 5.0, -7.0);
    let p = P::new(3.0, 4.0);
    let q = t.inverse().apply(t.apply(p));
    assert!(q.dist(p) < 1e-9);
    assert!((t.scale() - 2.0).abs() < 1e-12 && (t.angle() - 0.3).abs() < 1e-12);
    let u = Similarity::new(0.5, -0.1, 1.0, 2.0);
    assert!(t.after(&u).apply(p).dist(t.apply(u.apply(p))) < 1e-9);
    // Rescaled: the same map between half-size copies.
    let r = t.rescaled(0.5, 0.5);
    assert!(
        r.apply(P::new(1.5, 2.0))
            .dist(P::new(t.apply(p).x * 0.5, t.apply(p).y * 0.5))
            < 1e-9
    );
}

#[test]
fn overlay_colours() {
    // Paper, this sheet's line, the other's line, both.
    let a = Grey::new(4, 1, vec![255, 0, 255, 0]);
    let b = Grey::new(4, 1, vec![255, 255, 0, 0]);
    let o = compose(&a, &b, View::Overlay);
    assert_eq!(&o[0..4], &[255, 255, 255, 255]);
    assert_eq!(&o[4..8], &[255, 0, 0, 255], "this sheet only: red");
    assert_eq!(&o[8..12], &[0, 255, 255, 255], "the other only: cyan");
    assert_eq!(&o[12..16], &[0, 0, 0, 255], "both: black");
    let d = compose(&a, &b, View::Difference);
    assert_eq!([d[0], d[4], d[8], d[12]], [255, 0, 0, 255]);
    let h = compose(&a, &b, View::Blend(0.5));
    assert_eq!([h[0], h[4], h[8], h[12]], [255, 128, 128, 0]);
    assert_eq!(compose(&a, &b, View::Blend(0.0))[4], 0);
    assert_eq!(compose(&a, &b, View::Blend(1.0))[4], 255);
}

#[test]
fn changed_regions_are_found() {
    let a = synthetic_sheet(800, 600);
    let mut b = a.clone();
    // A wall added in one place and a mark erased in another.
    for y in 200..204 {
        for x in 400..520 {
            b.px[y * 800 + x] = 0;
        }
    }
    let mut erased = 0;
    for y in 450..520 {
        for x in 100..200 {
            if b.px[y * 800 + x] == 0 {
                erased += 1;
            }
            b.px[y * 800 + x] = 255;
        }
    }
    // Shifted by half a pixel (rendering differences) and still no false changes elsewhere.
    let half = warp(&b, &Similarity::new(1.0, 0.0, 0.4, 0.0), 800, 600);
    let found = changes(&a, &half, 2, 12, 20);
    let hit = |r: &[f64; 4], x: f64, y: f64| r[0] <= x && x <= r[2] && r[1] <= y && y <= r[3];
    assert!(found.iter().any(|r| hit(r, 460.0, 202.0)), "{found:?}");
    if erased > 20 {
        assert!(found.iter().any(|r| hit(r, 150.0, 485.0)), "{found:?}");
    }
    assert!(found.len() <= 2 + usize::from(erased > 20), "{found:?}");
    assert!(changes(&a, &a, 2, 12, 20).is_empty());
}

#[test]
fn an_image_sheet_exports_with_its_markup() {
    // A 400 × 300 grey image, a cloud in pixels.
    let rgb = vec![200u8; 400 * 300 * 3];
    let (base, k) = pdf::image_sheet(&rgb, 400, 300);
    assert_eq!(k, 1.0);
    let mut d = Doc::new(Space::Pixels);
    let mut m = sample("x").remove(0);
    m.shape = Shape::Cloud(vec![
        P::new(50.0, 50.0),
        P::new(250.0, 50.0),
        P::new(250.0, 150.0),
    ]);
    d.edit(0, |v| v.push(m));
    let out = pdf::export(&base, &d, k, &words, None).unwrap();
    let (w, h, px) = render(&out);
    assert_eq!((w, h), (400, 300));
    // The image's grey, and the cloud's red over it.
    assert_eq!(&px[(250 * 400 + 350) * 4..][..3], &[200, 200, 200]);
    assert!(reddish(&px, w, 150.0, 46.0, 3));
    // A very wide image keeps the page under 200 inches.
    let (wide, k) = pdf::image_sheet(&vec![0; 20_000 * 3], 20_000, 1);
    assert!((k - 0.72).abs() < 1e-9);
    let d = specular_pdf::Document::open(Arc::new(wide)).unwrap();
    assert!((d.page_size(0).unwrap().0 - 14_400.0).abs() < 0.5);
}

/// A plan sheet and its revision (turned 1.8° about the page's corner, shifted 9, −6 pt, a
/// wall moved and a door added), both rendered from PDF as the app does: aligned to within a
/// pixel of the copy, and the changes found.
#[test]
fn a_pdf_revision_aligns() {
    fn sheet(moved: bool) -> String {
        let x = if moved { 460 } else { 400 };
        let mut s = format!(
            "0 0 0 RG 2 w 20 20 802 555 re S 1.5 w 100 100 m 700 100 l S 100 100 m 100 480 l S \
             100 480 m 700 480 l S 700 100 m 700 480 l S {x} 100 m {x} 480 l S \
             250 300 m 330 300 l S 560 140 m 620 140 l 620 200 l S \
             0.7 w 600 30 200 60 re S 610 70 m 790 70 l S"
        );
        if moved {
            s = format!("q 0.9995 0.0314 -0.0314 0.9995 9 -6 cm {s} 160 480 m 160 520 l S Q");
        }
        s
    }
    let objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 842 595] /Contents 4 0 R >>".to_owned(),
        specular_pdf::fixture::stream("", &sheet(false)),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 842 595] /Contents 6 0 R >>".to_owned(),
        specular_pdf::fixture::stream("", &sheet(true)),
    ];
    let doc = specular_pdf::Document::open(Arc::new(specular_pdf::fixture::raw(&objects))).unwrap();
    let k = 2048.0 / 842.0;
    let grey = |page: usize| {
        let (w, h) = ((842.0 * k) as u32, (595.0 * k) as u32);
        let req = specular_pdf::RenderRequest {
            page,
            scale_x: k,
            scale_y: k,
            x: 0,
            y: 0,
            width: w,
            height: h,
        };
        let px = doc.render(req, &|| false).unwrap();
        Grey::from_rgba(w as usize, h as usize, &px, w as usize * 4)
    };
    let (a, b) = (grey(0), grey(1));
    let got = auto(&a, &b, None).expect("aligned");
    // The revision's content in page 1's pixels: user (x, y) → page 2 user via the cm; both
    // pages map user to pixels by (x k, (595 − y) k).
    let cm = |x: f64, y: f64| (0.9995 * x - 0.0314 * y + 9.0, 0.0314 * x + 0.9995 * y - 6.0);
    let px = |(x, y): (f64, f64)| P::new(x * k, (595.0 - y) * k);
    let mut worst = 0f64;
    for (x, y) in [(100.0, 100.0), (700.0, 480.0), (20.0, 575.0), (822.0, 20.0)] {
        let on_b = px(cm(x, y));
        let on_a = px((x, y));
        worst = worst.max(got.t.apply(on_b).dist(on_a));
    }
    eprintln!(
        "pdf revision: worst {worst:.3} px, rms {:.3}, {} matches",
        got.rms, got.matches
    );
    assert!(got.matches >= 3, "{got:?}");
    assert!(worst < 1.0, "{worst}");
    // The old wall, the new wall and the door; nothing else.
    let w = warp(&b, &got.t, a.w, a.h);
    let found = changes(&a, &w, 2, 14, 24);
    eprintln!("pdf revision: {} changed regions", found.len());
    assert_eq!(found.len(), 3, "{found:?}");
}

/// Sets text as a viewer would for the export: Hebrew with the bundled Hebrew UI font's real
/// glyphs, right to left (each line here is all Hebrew, so reversing it is its visual
/// order); anything the font lacks (CJK) as a box 1 em wide, as a fallback font would set it.
fn hebrew_outliner() -> impl Fn(&str, f64, bool) -> Option<pdf::Outlined> {
    let font = specular_font::open(
        include_bytes!("../../specular-theme/fonts/NotoSansHebrew-Variable.ttf"),
        specular_font::Limits::default(),
    )
    .unwrap();
    let face = font.faces[0].clone();
    move |s: &str, size: f64, _bold: bool| {
        use pdf::PathEl as E;
        use specular_font::PathEl as F;
        let k = size / f64::from(face.metrics.units_per_em);
        let rtl = s.chars().any(|c| ('\u{590}'..='\u{5FF}').contains(&c));
        let chars: Vec<char> = if rtl {
            s.chars().rev().collect()
        } else {
            s.chars().collect()
        };
        let (mut x, mut path) = (0.0, Vec::new());
        for c in chars {
            match face.glyph_index(c).filter(|g| *g != 0) {
                Some(g) => {
                    let p = |a: f32, b: f32| P::new(x + f64::from(a) * k, -f64::from(b) * k);
                    for e in face.outline(g).map(|o| o.path).unwrap_or_default() {
                        path.push(match e {
                            F::MoveTo(a, b) => E::Move(p(a, b)),
                            F::LineTo(a, b) => E::Line(p(a, b)),
                            F::QuadTo(c, d, a, b) => E::Quad(p(c, d), p(a, b)),
                            F::CubicTo(c, d, e, f, a, b) => E::Cubic(p(c, d), p(e, f), p(a, b)),
                            F::Close => E::Close,
                        });
                    }
                    x += f64::from(face.advance(g).unwrap_or(0)) * k;
                }
                None if c == ' ' => x += size * 0.3,
                None => {
                    let (x0, x1, y0, y1) = (x + size * 0.1, x + size * 0.9, -size * 0.8, 0.0);
                    path.extend([
                        E::Move(P::new(x0, y0)),
                        E::Line(P::new(x1, y0)),
                        E::Line(P::new(x1, y1)),
                        E::Line(P::new(x0, y1)),
                        E::Close,
                    ]);
                    x += size;
                }
            }
        }
        Some(pdf::Outlined { path, width: x })
    }
}

/// Reddish pixels in the box `x0..x1 × y0..y1`.
fn reddish_in(px: &[u8], w: u32, x0: f64, y0: f64, x1: f64, y1: f64) -> usize {
    let mut n = 0;
    for y in y0 as u32..y1 as u32 {
        for x in x0 as u32..x1 as u32 {
            let i = ((y * w + x) * 4) as usize;
            if let Some(p) = px.get(i..i + 3)
                && p[0] > 150
                && i32::from(p[0]) - i32::from(p[1]) > 60
                && i32::from(p[0]) - i32::from(p[2]) > 60
            {
                n += 1;
            }
        }
    }
    n
}

/// Hebrew and CJK labels and stamps export as outlines, not `?` (they printed `?` in
/// Helvetica), inside the annotation's box, and hayro draws them.
#[test]
fn hebrew_and_cjk_text_export_as_outlines() {
    let src = sheet_pdf(0);
    let red = [229, 57, 53];
    let mut text = Markup::new(Shape::Text(P::new(150.0, 150.0)), red, 1.5, 24.0, "x");
    text.text = "שלום עולם".into();
    let at = P::new(450.0, 300.0);
    let kind = Stamp::Custom;
    let mut stamp = Markup::new(Shape::Stamp { at, kind }, red, 1.5, 24.0, "דנה");
    stamp.text = "已批准".into();
    let mut d = Doc::new(Space::PdfPage);
    d.edit(0, |v| *v = vec![text.clone(), stamp.clone()]);

    // Without a text system: the old `?`.
    let plain = pdf::export(&src, &d, 1.0, &words, None).unwrap();
    assert!(plain.windows(3).any(|w| w == b"(??"));

    let ol = hebrew_outliner();
    let out = pdf::export(&src, &d, 1.0, &words, Some(&ol)).unwrap();
    assert!(!out.windows(2).any(|w| w == b"(?"), "no `?` drawn");
    // `/Contents` keeps the text (UTF-16).
    let utf16 = |s: &str| {
        s.encode_utf16()
            .map(|u| format!("{u:04X}"))
            .collect::<String>()
    };
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains(&utf16("שלום עולם")) && s.contains(&utf16("已批准")));

    let (w, _, px) = render(&out);
    // The Hebrew line, from (150, 150): ink all along it, not clipped by a box measured in
    // Helvetica.
    let hw = ol("שלום עולם", 24.0, false).unwrap().width;
    assert!(hw > 80.0, "{hw}");
    assert!(reddish_in(&px, w, 150.0, 150.0, 150.0 + hw, 180.0) > 150);
    assert!(reddish_in(&px, w, 150.0, 150.0, 170.0, 180.0) > 10);
    assert!(reddish_in(&px, w, 150.0 + hw - 20.0, 150.0, 150.0 + hw, 180.0) > 10);
    // The stamp's three CJK boxes, 1 em each, the outer two drawn whole.
    assert_eq!(ol("已批准", 24.0, true).unwrap().width, 72.0);
    let (fw, _) = stamp_extent_with(&words(&stamp), &stamp.stamp_line(), 24.0, &|t, z, b| {
        ol(t, z, b).unwrap().width
    });
    assert!(fw > 72.0);
    assert!(reddish_in(&px, w, at.x + 14.0, 285.0, at.x + 34.0, 300.0) > 100);
    assert!(reddish_in(&px, w, at.x - 34.0, 285.0, at.x - 14.0, 300.0) > 100);
}

/// A small thermal report: a 40 × 30 ramp from blue to red with a spot and a rectangle, a
/// grey photo, facts and a table; `title` in either script.
fn thermal_report(title: &str, rtl: bool, pages: usize) -> report::Report {
    let (w, h) = (40u32, 30u32);
    let mut rgb = Vec::new();
    for _y in 0..h {
        for x in 0..w {
            let t = (x * 255 / (w - 1)) as u8;
            rgb.extend_from_slice(&[t, 0, 255 - t]);
        }
    }
    let entry = report::Entry {
        title: "IR_0001.jpg".into(),
        thermal: report::Image {
            rgb,
            width: w,
            height: h,
        },
        marks: vec![
            report::Mark {
                points: vec![(30.0, 10.0)],
                closed: false,
                label: "1".into(),
            },
            report::Mark {
                points: vec![(5.0, 5.0), (15.0, 5.0), (15.0, 20.0), (5.0, 20.0)],
                closed: true,
                label: "2".into(),
            },
        ],
        scale: Some(report::Scale {
            colours: (0..=255u8).map(|t| [t, 0, 255 - t]).collect(),
            low: "12.4 °C".into(),
            high: "48.0 °C".into(),
        }),
        visible: Some(report::Image {
            rgb: vec![128; 20 * 15 * 3],
            width: 20,
            height: 15,
        }),
        thermal_caption: "Thermal image".into(),
        visible_caption: "Visible photo".into(),
        facts: vec![
            ("Camera".into(), "FLIR E8".into()),
            ("Emissivity".into(), "0.95".into()),
        ],
        table: vec![
            vec![
                "#".into(),
                "Kind".into(),
                "Min".into(),
                "Max".into(),
                "Mean".into(),
            ],
            vec![
                "1".into(),
                "Spot".into(),
                "40.1 °C".into(),
                "40.1 °C".into(),
                "40.1 °C".into(),
            ],
            vec![
                "2".into(),
                "Rectangle".into(),
                "12.4 °C".into(),
                "30.2 °C".into(),
                "21.0 °C".into(),
            ],
        ],
        no_table: "No measurements".into(),
    };
    report::Report {
        title: title.into(),
        subtitle: "2026-10-25 · Dana".into(),
        notes: "Roof, north side. Moisture suspected near the gutter.".into(),
        notes_heading: "Notes".into(),
        table_heading: "Measurements".into(),
        entries: vec![entry; pages],
        page_labels: (1..=pages)
            .map(|i| format!("Page {i} of {pages}"))
            .collect(),
        rtl,
        created: Some("D:20261025120000Z".into()),
        producer: "Specular test".into(),
    }
}

/// With `SPECULAR_REPORT_DUMP` set to a folder: the rendered page as a PPM, for a look.
fn dump(name: &str, w: u32, px: &[u8]) {
    let Ok(dir) = std::env::var("SPECULAR_REPORT_DUMP") else {
        return;
    };
    let h = px.len() as u32 / 4 / w;
    let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
    for p in px.chunks_exact(4) {
        out.extend_from_slice(&p[..3]);
    }
    std::fs::write(std::path::Path::new(&dir).join(format!("{name}.ppm")), out).unwrap();
}

/// Pixels in the box whose red and blue both differ from white (the ramp drew there).
fn inked_in(px: &[u8], w: u32, x0: f64, y0: f64, x1: f64, y1: f64) -> usize {
    let mut n = 0;
    for y in y0 as u32..y1 as u32 {
        for x in x0 as u32..x1 as u32 {
            let i = ((y * w + x) * 4) as usize;
            if let Some(p) = px.get(i..i + 3)
                && (p[0] < 200 || p[2] < 200)
            {
                n += 1;
            }
        }
    }
    n
}

/// The report renders through hayro: pages, the thermal image on the start side (left in
/// English, right in Hebrew), the photo beside it, text as Helvetica (searchable) or, for
/// Hebrew, as outlines with no `?`.
#[test]
fn thermal_report_renders_back() {
    let r = thermal_report("Thermal inspection", false, 2);
    let pdf = report::write(&r, None);
    let doc = specular_pdf::Document::open(Arc::new(pdf.clone())).unwrap();
    assert_eq!(doc.page_count(), 2);
    assert_eq!(doc.page_size(0), Some((595.0, 842.0)));
    let s = String::from_utf8_lossy(&pdf);
    assert!(s.contains("/Title (Thermal inspection)"));
    let (w, _, px) = render(&pdf);
    dump("report-en", w, &px);
    // The thermal image in the left half under the header, the photo in the right half.
    let left = inked_in(&px, w, 40.0, 100.0, 290.0, 300.0);
    let right = inked_in(&px, w, 305.0, 100.0, 555.0, 300.0);
    assert!(left > 10_000, "{left}");
    assert!(right > 10_000, "{right}");
    // The colour scale's red end is to the right of its blue end.
    let ramp_row = (100..520)
        .find(|&y| {
            let i = ((y * w + 50) * 4) as usize;
            px[i + 2] > 200 && px[i] < 60
        })
        .expect("the thermal image's blue edge");
    let at = |x: u32| {
        let i = ((ramp_row * w + x) * 4) as usize;
        (px[i], px[i + 2])
    };
    assert!(at(280).0 > at(50).0, "{:?} {:?}", at(50), at(280));

    // Hebrew: outlines, mirrored.
    let ol = hebrew_outliner();
    let he = thermal_report("דוח תרמי", true, 1);
    let pdf = report::write(&he, Some(&ol));
    assert!(!pdf.windows(2).any(|w| w == b"(?"), "no `?` drawn");
    let (w, _, px) = render(&pdf);
    dump("report-he", w, &px);
    // The thermal image now on the right: its blue (cold) edge stays at the reading start,
    // the image itself is not mirrored.
    let right = inked_in(&px, w, 305.0, 100.0, 555.0, 300.0);
    assert!(right > 10_000, "{right}");
    // The title's outlines at the top right, nothing at the top left.
    let title_r = inked_in(&px, w, 400.0, 40.0, 555.0, 62.0);
    let title_l = inked_in(&px, w, 40.0, 40.0, 200.0, 62.0);
    assert!(title_r > 50 && title_l == 0, "{title_r} {title_l}");
}

/// Long cells are shortened to their column, an empty table says so, and many facts stop
/// at the page's foot.
#[test]
fn thermal_report_fits_its_page() {
    let mut r = thermal_report("Report", false, 1);
    r.entries[0].table = vec![vec!["#".into()]];
    r.entries[0].facts = (0..200)
        .map(|i| (format!("Fact {i}"), "x".repeat(400)))
        .collect();
    let pdf = report::write(&r, None);
    let doc = specular_pdf::Document::open(Arc::new(pdf.clone())).unwrap();
    assert_eq!(doc.page_count(), 1);
    let (w, h, px) = render(&pdf);
    // Nothing drawn in the bottom margin but the page label.
    let foot = inked_in(&px, w, 0.0, f64::from(h) - 30.0, f64::from(w), f64::from(h));
    assert!(foot < 400, "{foot}");
}
