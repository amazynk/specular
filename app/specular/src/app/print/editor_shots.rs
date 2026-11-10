//! Screenshot tests of the free layout editor, dark and light, in English and in Hebrew
//! (right to left: the controls mirror, the page does not): a 4-up page rendered by the real
//! page renderer, an area selected with its handles, a guide it snapped to, one photo too
//! small for its area (flagged under 200 dpi), and a layout name being typed. Rendered
//! offscreen on lavapipe and compared with the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular print::editor_shots`.

use super::shots::Photos;
use super::*;
use specular_print::free::{self, Area};
use specular_print::layout::RectMm;
use specular_shots::{Goldens, Harness};

const SIZE: (f32, f32) = (900.0, 620.0);

fn backdrop(dl: &mut DrawList) {
    dl.rect(
        Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
        0.0,
        Color::rgba(0.2, 0.32, 0.45, 1.0),
    );
}

fn free_choices() -> Choices {
    Choices {
        sheet: false,
        free: true,
        free_unit: Unit::Mm,
        setup: PageSetup::default(),
        unit: Unit::Cm,
        fit: Fit::Fit,
        width: 100.0,
        ratio: None,
        single_caption: Caption::Off,
        cols: 4,
        rows: 6,
        header: true,
        sheet_caption: Caption::Name,
        copies: 1,
    }
}

/// The shipped 4-up layout, its last area made larger than its photo's pixels allow.
fn four_up() -> FreeLayout {
    let mut l = free::builtin(Paper::A4).remove(2);
    l.areas[3].placement = free::Placement::Fit;
    l
}

/// Photos of 3000 × 2000 px, the last one 300 × 200 (too few pixels).
fn items() -> Vec<Item> {
    (0..4)
        .map(|i| Item {
            size: if i == 3 { (300, 200) } else { (3000, 2000) },
            name: format!("IMG_{i}.jpg"),
            ..Item::default()
        })
        .collect()
}

/// The dialog's view of `l` with area `selected`, built as the print state builds it.
fn view(l: &FreeLayout, selected: Option<usize>, rtl: bool, typing: Option<&str>) -> DialogView {
    let c = free_choices();
    let mut rows = row_views(&c, source_label(Source::Selection, 4));
    free_rows(
        &mut rows,
        &c,
        l,
        selected,
        &layout_name(l),
        true,
        SnapMode::Guides,
    );
    if let Some(t) = typing
        && let Some(r) = rows.iter_mut().find(|r| r.row == Row::Saved)
    {
        r.value = t.to_owned();
        r.editing = Some(t.len());
        r.label = t!("print-layout-name");
    }
    let order = l.fill_order(rtl);
    let its = items();
    let areas = l
        .areas
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let k = order.iter().position(|&o| o == i).unwrap_or(i);
            let photo = its.get(k).map(|it| (it.size, free::place(a, it.size).0));
            editor::area_view(k + 1, a.rect, photo, c.free_unit, selected == Some(i))
        })
        .collect();
    let (pw, ph) = l.sheet_mm();
    DialogView {
        rows,
        page: (1, 1),
        aspect: (pw / ph) as f32,
        preview: None,
        status: None,
        progress: None,
        busy: false,
        can_print: true,
        focus: None,
        editor: Some(EditorView {
            sheet: (pw, ph),
            printable: l.printable(),
            areas,
            guides: vec![Guide::X(pw * 0.5)],
            grid: None,
            notice: l.borderless.then(|| t!("print-borderless-note")),
            tools: vec![
                (Hit::AddArea, t!("print-area-add"), true),
                (Hit::RemoveArea, t!("print-area-remove"), selected.is_some()),
                (Hit::SaveLayout, t!("print-layout-save"), true),
                (Hit::DeleteLayout, t!("print-layout-delete"), false),
            ],
        }),
    }
}

#[test]
fn free_layout_editor_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-print-editor",
    );
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    let mut r =
        specular_print::PageRenderer::new(h.gpu.clone(), specular_gfx::Text::bundled_only());
    let l = four_up();
    let mut borderless = l.clone();
    borderless.set_margin(borderless.margin, true);
    for (name, lang, rtl, pal, layout, typing) in [
        ("dark", en.clone(), false, Palette::dark(), &l, None),
        (
            "light-typing",
            en.clone(),
            false,
            Palette::light(),
            &l,
            Some("Family 4-up"),
        ),
        ("he-dark", he.clone(), true, Palette::dark(), &l, None),
        (
            "he-light-borderless",
            he.clone(),
            true,
            Palette::light(),
            &borderless,
            None,
        ),
    ] {
        let doc = layout.document(300, &items(), rtl);
        let (pw, ph, rgba) = r
            .render_rgba(&doc, 0, 50.0, &Photos, &|| false)
            .expect("page");
        let (_tex, tex) = upload(&h.gpu, &mut h.ui, pw, ph, &rgba);
        let (shot, tree) = specular_i18n::I18n::with_thread_language(lang, || {
            let mut v = view(layout, Some(1), rtl, typing);
            v.preview = Some(tex);
            v.focus = v.focusables().iter().position(|x| *x == Hit::Page);
            let mut drawn = None;
            let shot = h.render(SIZE, 1.0, backdrop, |dl, text| {
                drawn = Some(draw(dl, text, &pal, &v, SIZE, rtl, 1.0));
            });
            let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
            a11y(&mut tree, &v, &drawn.expect("drawn"), 1.0);
            (shot, tree)
        });
        goldens.check(&format!("print-editor-{name}-1x"), &shot);
        super::super::a11y_audit::expect_clean(&format!("print-editor-{name}"), &tree, rtl);
    }
    goldens.finish();
}

/// The editor's controls: the toolbar and page come before the rows in the focus order
/// (the preview column reads first); areas are screen-reader buttons that select them; the page is not
/// mirrored in a right-to-left language while the controls are.
#[test]
fn editor_focus_order_and_mirroring() {
    let Some(mut h) = Harness::new() else { return };
    let l = four_up();
    let v = view(&l, Some(0), false, None);
    let f = v.focusables();
    let n = v.rows.len();
    assert_eq!(
        f[..4],
        [Hit::AddArea, Hit::RemoveArea, Hit::SaveLayout, Hit::Page],
        "Delete layout is skipped when it cannot be used"
    );
    assert_eq!(
        f[4..4 + n],
        v.rows.iter().map(|r| Hit::Row(r.row)).collect::<Vec<_>>()
    );
    assert_eq!(f[4 + n..], [Hit::Cancel, Hit::Save, Hit::Print]);
    assert!(v.rows.iter().any(|r| r.row == Row::Placement));
    assert!(v.rows.iter().any(|r| r.row == Row::Borderless));
    let pal = Palette::dark();
    let (mut ltr, mut rtl) = (None, None);
    h.render(SIZE, 1.0, backdrop, |dl, text| {
        ltr = Some(draw(dl, text, &pal, &v, SIZE, false, 1.0));
        rtl = Some(draw(dl, text, &pal, &v, SIZE, true, 1.0));
    });
    let (ltr, rtl) = (ltr.unwrap(), rtl.unwrap());
    assert!(ltr.page.right() <= ltr.rows[0].0.x, "page before the rows");
    assert!(
        rtl.page.x >= rtl.rows[0].0.right(),
        "rows before the page in RTL"
    );
    // The toolbar mirrors.
    let add = |d: &Drawn| d.hits.iter().find(|(_, h)| *h == Hit::AddArea).unwrap().0;
    assert!((add(&ltr).x - (SIZE.0 - add(&rtl).right())).abs() < 0.01);
    // Areas, as a screen reader finds them, select their area.
    let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
    a11y(&mut tree, &v, &ltr, 1.0);
    let areas: Vec<_> = tree
        .nodes
        .iter()
        .filter(|(id, _)| (A11Y_BASE + 0x200..A11Y_BASE + 0x300).contains(&id.0))
        .collect();
    assert_eq!(areas.len(), 4);
    assert_eq!(a11y_target(&v, &ltr, A11Y_BASE + 0x202), Some(Hit::Area(2)));
    assert_eq!(a11y_target(&v, &ltr, A11Y_BASE + 0x180), Some(Hit::AddArea));
    // The low-resolution area says so.
    let low = &v.editor.as_ref().unwrap().areas[3];
    assert!(low.low, "{}", low.label);
}

/// Photos land in the areas in selection order; the free layout's document is what the
/// dialog prints.
#[test]
fn photos_fill_areas_in_selection_order() {
    let c = free_choices();
    let l = four_up();
    let its: Vec<Item> = (0..6)
        .map(|i| Item {
            size: (300, 200),
            name: format!("{i}"),
            ..Item::default()
        })
        .collect();
    let doc = document(&c, &its, "t", false, Some(&l));
    assert_eq!(doc.pages.len(), 2);
    let first: Vec<usize> = doc.pages[0].images.iter().map(|p| p.item).collect();
    assert_eq!(first, [0, 1, 2, 3]);
    // Reading order: top left, top right, bottom left, bottom right.
    let r = |k: usize| doc.pages[0].images[k].clip;
    assert!(r(0).x < r(1).x && (r(0).y - r(1).y).abs() < 1e-6);
    assert!(r(2).y > r(0).y && (r(2).x - r(0).x).abs() < 1e-6);
    assert_eq!(doc.pages[1].images.len(), 2);
    // Without a free layout the choices' own layout prints.
    let mut c2 = c;
    c2.free = false;
    let doc = document(&c2, &its, "t", false, Some(&l));
    assert_eq!(doc.pages.len(), 6);
    let _ = RectMm::default();
    let _ = Area::new(RectMm::default(), free::Placement::Fit);
}

/// Black point compensation: on by default for relative colorimetric, switchable;
/// perceptual always compensates.
#[test]
fn black_point_compensation_follows_the_switch() {
    use specular_color::transform::Intent;
    let icc =
        include_bytes!("../../../../../crates/specular-color/fuzz/seeds/profile/cmyk_v2_lut16.icc");
    let mut dp = DevelopPrint {
        path: PathBuf::from("/x.jpg"),
        edit: specular_develop::model::Edit::default_for(
            specular_develop::model::Registry::builtin(),
            specular_develop::model::SourceKey::default(),
            false,
        ),
        size: (3000, 2000),
        profile: Some(("CMYK".into(), Arc::new(icc.to_vec()))),
        through: true,
        relative: true,
        bpc: true,
        proof: true,
        view: specular_develop::proof::ProofSettings::default(),
        styles: Vec::new(),
        style: None,
    };
    let rows = |dp: &DevelopPrint| dp.rows().iter().map(|r| r.row).collect::<Vec<_>>();
    assert!(rows(&dp).contains(&Row::Bpc));
    let on = dp.output().unwrap().unwrap();
    assert_eq!((on.intent, on.bpc), (Intent::Relative, true));
    dp.bpc = false;
    let off = dp.output().unwrap().unwrap();
    assert_eq!((off.intent, off.bpc), (Intent::Relative, false));
    dp.relative = false;
    assert!(!rows(&dp).contains(&Row::Bpc), "no switch for perceptual");
    let p = dp.output().unwrap().unwrap();
    assert_eq!((p.intent, p.bpc), (Intent::Perceptual, true));
    // The preview proofs only when asked.
    assert!(dp.filter().is_some());
    dp.proof = false;
    assert!(dp.filter().is_none());
    dp.through = false;
    assert!(dp.output().is_none());
    assert!(!rows(&dp).contains(&Row::Intent));
    assert!(rows(&dp).contains(&Row::Style));
}
