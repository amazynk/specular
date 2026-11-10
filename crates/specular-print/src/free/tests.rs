use super::*;
use crate::layout::Unit;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn rect_close(a: RectMm, b: RectMm) -> bool {
    close(a.x, b.x) && close(a.y, b.y) && close(a.w, b.w) && close(a.h, b.h)
}

/// A4 portrait, 10 mm margins, 5 mm gap, one area at (50, 50) 60×40.
fn one_area() -> FreeLayout {
    FreeLayout {
        name: "t".into(),
        areas: vec![Area::new(
            RectMm::new(50.0, 50.0, 60.0, 40.0),
            Placement::Fill,
        )],
        ..FreeLayout::default()
    }
}

const GUIDES: Snap = Snap {
    threshold: 2.0,
    guides: true,
    grid: None,
};

#[test]
fn moving_snaps_to_the_margin_and_the_centre() {
    let l = one_area();
    let start = l.areas[0].rect;
    // Left edge to 11.5 mm: within 2 mm of the 10 mm margin.
    let (r, g) = drag(&l, 0, start, Handle::Move, (-38.5, 0.0), &GUIDES);
    assert!(close(r.x, 10.0), "{r:?}");
    assert!(close(r.w, 60.0) && close(r.y, 50.0));
    assert_eq!(g, vec![Guide::X(10.0)]);
    // Centre to 104 mm: snaps to the sheet's centre line (105).
    let (r, g) = drag(&l, 0, start, Handle::Move, (24.0, 0.0), &GUIDES);
    assert!(close(r.x + r.w * 0.5, 105.0), "{r:?}");
    assert_eq!(g, vec![Guide::X(105.0)]);
    // Right edge near the right margin (200).
    let (r, _) = drag(&l, 0, start, Handle::Move, (88.6, 0.0), &GUIDES);
    assert!(close(r.right(), 200.0), "{r:?}");
    // Far from everything: no snap, no guide.
    let (r, g) = drag(&l, 0, start, Handle::Move, (7.3, 3.3), &GUIDES);
    assert!(rect_close(r, RectMm::new(57.3, 53.3, 60.0, 40.0)), "{r:?}");
    assert!(g.is_empty());
    // Snapping off: exactly where it was dragged.
    let (r, _) = drag(&l, 0, start, Handle::Move, (-38.5, 0.0), &Snap::OFF);
    assert!(close(r.x, 11.5));
}

#[test]
fn moving_snaps_to_other_areas_and_the_gap() {
    let mut l = one_area();
    l.areas.push(Area::new(
        RectMm::new(120.0, 50.0, 60.0, 40.0),
        Placement::Fit,
    ));
    let start = l.areas[1].rect;
    // Its left edge to 116.5: the first area's right (110) + gap (5) = 115 is nearest.
    let (r, g) = drag(&l, 1, start, Handle::Move, (-3.5, 0.0), &GUIDES);
    assert!(close(r.x, 115.0), "{r:?}");
    assert_eq!(
        g,
        vec![Guide::X(115.0), Guide::Y(50.0)],
        "the tops line up too"
    );
    // Edge to edge.
    let (r, _) = drag(&l, 1, start, Handle::Move, (-9.2, 0.0), &GUIDES);
    assert!(close(r.x, 110.0), "{r:?}");
    // Tops line up.
    let (r, g) = drag(&l, 1, start, Handle::Move, (0.0, 1.5), &GUIDES);
    assert!(close(r.y, 50.0));
    assert!(g.contains(&Guide::Y(50.0)));
}

#[test]
fn resizing_snaps_one_edge_and_keeps_the_other() {
    let l = one_area();
    let start = l.areas[0].rect;
    let (r, g) = drag(&l, 0, start, Handle::E, (88.8, 0.0), &GUIDES);
    assert!(close(r.x, 50.0) && close(r.right(), 200.0), "{r:?}");
    assert_eq!(g, vec![Guide::X(200.0)]);
    let (r, _) = drag(&l, 0, start, Handle::NW, (-39.0, -41.0), &GUIDES);
    assert!(rect_close(r, RectMm::new(10.0, 10.0, 100.0, 80.0)), "{r:?}");
    // Never smaller than the smallest area, never off the sheet.
    let (r, _) = drag(&l, 0, start, Handle::W, (500.0, 0.0), &Snap::OFF);
    assert!(close(r.w, MIN_AREA_MM) && close(r.right(), 110.0), "{r:?}");
    let (r, _) = drag(&l, 0, start, Handle::SE, (900.0, 900.0), &Snap::OFF);
    assert!(close(r.right(), 210.0) && close(r.bottom(), 297.0), "{r:?}");
}

#[test]
fn the_grid_takes_over_where_no_guide_is_near() {
    let l = one_area();
    let start = l.areas[0].rect;
    let snap = Snap {
        threshold: 2.0,
        guides: true,
        grid: Some(MM_PER_IN / 4.0),
    };
    // 57.3 → the nearest quarter inch, 57.15 mm.
    let (r, g) = drag(&l, 0, start, Handle::Move, (7.3, 0.0), &snap);
    assert!(close(r.x, 9.0 * MM_PER_IN / 4.0), "{r:?}");
    assert!(g.is_empty());
    // A margin within reach still wins over the grid.
    let (r, _) = drag(&l, 0, start, Handle::Move, (-38.5, 0.0), &snap);
    assert!(close(r.x, 10.0));
}

#[test]
fn lengths_convert_between_millimetres_and_inches() {
    assert!(close(Unit::In.parse("1").unwrap(), 25.4));
    assert!(close(Unit::In.parse("0,25").unwrap(), 6.35));
    assert!(close(Unit::Mm.parse("12.5").unwrap(), 12.5));
    assert!(close(Unit::Mm.parse("12,5 mm").unwrap(), 12.5));
    assert!(close(Unit::Mm.parse("1 in").unwrap(), 25.4));
    assert!(close(Unit::Mm.parse("2\"").unwrap(), 50.8));
    assert!(close(Unit::In.parse("1.5cm").unwrap(), 15.0));
    for bad in ["", "abc", "-3", "1.2.3", "3 ft", "99999"] {
        assert_eq!(Unit::Mm.parse(bad), None, "{bad}");
    }
    assert_eq!(Unit::Mm.format(12.5), "12.5");
    assert_eq!(Unit::Mm.format(10.0), "10");
    assert_eq!(Unit::In.format(6.35), "0.25");
    assert_eq!(Unit::In.format(Unit::In.parse("0.75").unwrap()), "0.75");
    assert_eq!(Unit::for_locale("en_US.UTF-8"), Unit::In);
    assert_eq!(Unit::for_locale("he_IL.UTF-8"), Unit::Mm);
    assert_eq!(Unit::for_locale("en_GB"), Unit::Mm);
    assert_eq!(Unit::Mm.next(1), Unit::Cm);
    assert_eq!(Unit::Mm.next(-1), Unit::In);
}

#[test]
fn fit_fill_and_the_crop_pan() {
    let mut a = Area::new(RectMm::new(10.0, 20.0, 100.0, 100.0), Placement::Fit);
    // A 3:2 landscape photo fitted: 100 × 66.67, centred.
    let (img, clip) = place(&a, (3000, 2000));
    assert!(rect_close(
        img,
        RectMm::new(10.0, 20.0 + 50.0 / 3.0, 100.0, 200.0 / 3.0)
    ));
    assert_eq!(img, clip);
    // Filled: 150 × 100, centred, clipped to the area.
    a.placement = Placement::Fill;
    let (img, clip) = place(&a, (3000, 2000));
    assert!(
        rect_close(img, RectMm::new(-15.0, 20.0, 150.0, 100.0)),
        "{img:?}"
    );
    assert_eq!(clip, a.rect);
    // Pan to the left part, then the right part.
    a.pan = (-1.0, 0.0);
    assert!(close(place(&a, (3000, 2000)).0.x, 10.0));
    a.pan = (1.0, 0.0);
    assert!(close(place(&a, (3000, 2000)).0.right(), 110.0));
    // Dragging the photo 25 mm right from the right part brings it back to the middle;
    // down does nothing (nothing to crop).
    let pan = pan_by(&a, (3000, 2000), (25.0, 10.0));
    assert!(close(pan.0, 0.0) && close(pan.1, 0.0), "{pan:?}");
    a.pan = (0.0, 0.0);
    let pan = pan_by(&a, (3000, 2000), (-1000.0, 0.0));
    assert!(close(pan.0, 1.0));
}

#[test]
fn effective_dpi_follows_the_printed_size() {
    // 3000 px over 10 inches: 300 dpi; filled photos count their whole printed width.
    assert!(close(
        effective_dpi((3000, 2000), &RectMm::new(0.0, 0.0, 254.0, 1.0)),
        300.0
    ));
    let a = Area::new(RectMm::new(0.0, 0.0, 254.0, 254.0), Placement::Fill);
    let (img, _) = place(&a, (3000, 2000));
    assert!(close(effective_dpi((3000, 2000), &img), 200.0));
    assert!(effective_dpi((1200, 800), &img) < LOW_DPI);
}

fn items(n: usize) -> Vec<Item> {
    (0..n)
        .map(|i| Item {
            size: (300, 200),
            name: format!("{i}.jpg"),
            ..Item::default()
        })
        .collect()
}

#[test]
fn photos_fill_the_areas_in_reading_order_page_after_page() {
    let mut l = FreeLayout::default();
    l.grid(2, 2, Placement::Fill);
    // Stored out of order: the order photos land in is still reading order.
    l.areas.swap(0, 3);
    let doc = l.document(300, &items(6), false);
    assert_eq!(doc.pages.len(), 2);
    let p = l.printable();
    let at = |page: usize, k: usize| doc.pages[page].images[k].clip;
    assert_eq!(
        doc.pages[0]
            .images
            .iter()
            .map(|i| i.item)
            .collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    assert!(
        close(at(0, 0).x, p.x) && close(at(0, 0).y, p.y),
        "first top left"
    );
    assert!(
        at(0, 1).x > at(0, 0).x && close(at(0, 1).y, p.y),
        "second top right"
    );
    assert!(at(0, 2).y > p.y + 1.0 && close(at(0, 2).x, p.x));
    assert_eq!(doc.pages[1].images.len(), 2);
    assert_eq!(doc.pages[1].images[0].item, 4);
    // Right to left: the first photo top right.
    let doc = l.document(300, &items(4), true);
    assert!(doc.pages[0].images[0].clip.x > doc.pages[0].images[1].clip.x);
    assert!(doc.rtl);
    assert_eq!(l.area_of(5, false).map(|x| x.0), Some(1));
    // No areas or no photos: one empty page.
    assert_eq!(
        FreeLayout::default()
            .document(300, &items(3), false)
            .pages
            .len(),
        1
    );
    assert!(l.document(300, &[], false).pages[0].images.is_empty());
}

#[test]
fn margins_clip_unless_borderless() {
    let mut l = FreeLayout {
        areas: vec![Area::new(
            RectMm::new(0.0, 0.0, 210.0, 297.0),
            Placement::Fill,
        )],
        ..FreeLayout::default()
    };
    let doc = l.document(300, &items(1), false);
    assert_eq!(doc.pages[0].images[0].clip, l.printable());
    assert!(close(doc.setup.margin, 10.0));
    l.borderless = true;
    let doc = l.document(300, &items(1), false);
    assert!(rect_close(
        doc.pages[0].images[0].clip,
        RectMm::new(0.0, 0.0, 210.0, 297.0)
    ));
    assert!(close(doc.setup.margin, 0.0));
}

#[test]
fn margin_lines_carry_their_edges() {
    let mut l = FreeLayout::default();
    l.grid(1, 1, Placement::Fit);
    l.areas.push(Area::new(
        RectMm::new(50.0, 50.0, 20.0, 20.0),
        Placement::Fit,
    ));
    l.set_margin(20.0, false);
    assert!(rect_close(
        l.areas[0].rect,
        RectMm::new(20.0, 20.0, 170.0, 257.0)
    ));
    assert!(rect_close(
        l.areas[1].rect,
        RectMm::new(50.0, 50.0, 20.0, 20.0)
    ));
    l.set_margin(20.0, true);
    assert!(rect_close(
        l.areas[0].rect,
        RectMm::new(0.0, 0.0, 210.0, 297.0)
    ));
    // A new sheet keeps the grid filling the printable area.
    let mut g = FreeLayout::default();
    g.grid(2, 1, Placement::Fill);
    g.set_sheet(Paper::A4, PageOrientation::Landscape);
    let p = g.printable();
    assert!(close(g.areas[0].rect.x, p.x) && close(g.areas[1].rect.right(), p.right()));
    assert!(close(g.areas[0].rect.bottom(), p.bottom()));
}

#[test]
fn layouts_save_and_load() {
    let mut l = FreeLayout {
        name: "Two portraits [mine]".into(),
        paper: Paper::Custom { w: 127.0, h: 177.8 },
        orientation: PageOrientation::Landscape,
        margin: 6.35,
        gap: 3.0,
        borderless: true,
        ..FreeLayout::default()
    };
    l.grid(2, 1, Placement::Fill);
    l.areas[1].pan = (-0.5, 0.25);
    l.areas[0].placement = Placement::Fit;
    let mut all = builtin(Paper::A4);
    all.push(l.clone());
    let text = write_all(&all);
    let back = parse_all(&text);
    assert_eq!(back.len(), all.len());
    for (a, b) in all.iter().zip(&back) {
        assert_eq!(a.id, b.id);
        assert_eq!(a.name, b.name);
        assert_eq!(a.paper, b.paper);
        assert_eq!(a.orientation, b.orientation);
        assert_eq!(a.borderless, b.borderless);
        assert!(close(a.margin, b.margin) && close(a.gap, b.gap));
        assert_eq!(a.areas.len(), b.areas.len());
        for (x, y) in a.areas.iter().zip(&b.areas) {
            assert!((x.rect.x - y.rect.x).abs() < 1e-3 && (x.rect.w - y.rect.w).abs() < 1e-3);
            assert_eq!(x.placement, y.placement);
            assert!((x.pan.0 - y.pan.0).abs() < 1e-3 && (x.pan.1 - y.pan.1).abs() < 1e-3);
        }
    }
    // Damaged and newer files load what they can.
    let odd = "# comment\n[]\npaper = a4\n[Kept]\npaper = tabloid\nmargin = -4\nfuture = 1\n\
               area = 1 2 3\narea = 10 10 50 50 fill 9 x\narea = 10 10 0 5\n";
    let back = parse_all(odd);
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].name, "Kept");
    assert_eq!(back[0].paper, Paper::A4);
    assert!(close(back[0].margin, 10.0));
    assert_eq!(back[0].areas.len(), 1);
    assert_eq!(back[0].areas[0].pan, (1.0, 0.0));
}

#[test]
fn shipped_layouts() {
    let all = builtin(Paper::Letter);
    let ids: Vec<&str> = all.iter().map(|l| l.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "one-border",
            "two-up",
            "four-up",
            "contact-3x3",
            "8x10-letter",
            "a4-mat"
        ]
    );
    let n: Vec<usize> = all.iter().map(|l| l.areas.len()).collect();
    assert_eq!(n, [1, 2, 4, 9, 1, 1]);
    for l in &all {
        let p = l.printable();
        for a in &l.areas {
            assert!(p.contains(&a.rect), "{} {:?}", l.id, a.rect);
        }
    }
    let ten = &all[4].areas[0].rect;
    assert!(close(ten.w, 203.2) && close(ten.h, 254.0));
    assert_eq!(all[4].paper, Paper::Letter);
    assert_eq!(all[5].paper, Paper::A4);
    // A mat: the bottom border is the widest.
    let m = all[5].areas[0].rect;
    let (_, h) = all[5].sheet_mm();
    assert!(h - m.bottom() > m.y);
}

#[test]
fn new_areas_do_not_stack_exactly() {
    let mut l = FreeLayout::default();
    let a = l.add_area();
    let b = l.add_area();
    assert_ne!(l.areas[a].rect, l.areas[b].rect);
    assert!(l.printable().contains(&l.areas[b].rect));
}
