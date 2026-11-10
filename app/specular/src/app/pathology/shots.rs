//! Screenshot tests of stain separation on the GPU: a synthetic H&E field (nuclei of
//! haematoxylin in eosin-stained stroma, Beer–Lambert mixed from the published vectors)
//! drawn by the image renderer with each stain as light and as optical density, under the
//! stains' legend, in English and Hebrew at 1× and 1.5×. Rendered offscreen on lavapipe and
//! compared with the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular pathology::shots`, then review the
//! rewritten PNGs.

use specular_core::{Orientation, PixelLayout, Raster, RasterPlane};
use specular_render::{ImageRenderer, PxRect, RenderOptions, View};
use specular_shots::{Goldens, Harness};

use super::*;

const SIZE: (f32, f32) = (560.0, 380.0);
const W: u32 = 280;
const H: u32 = 190;

/// The field: stroma with eosin rising left to right, round nuclei of haematoxylin (some
/// over stroma), and a bare corner of glass.
pub(crate) fn field() -> Raster {
    let s = Stains::published(StainSet::He);
    let nuclei: [(f64, f64, f64); 9] = [
        (40.0, 50.0, 14.0),
        (95.0, 30.0, 11.0),
        (150.0, 70.0, 16.0),
        (210.0, 45.0, 12.0),
        (60.0, 130.0, 13.0),
        (120.0, 150.0, 10.0),
        (185.0, 135.0, 15.0),
        (240.0, 110.0, 9.0),
        (255.0, 165.0, 12.0),
    ];
    let mut data = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        for x in 0..W {
            let (fx, fy) = (f64::from(x), f64::from(y));
            let glass = fx + fy * 1.4 > 380.0;
            let e = if glass {
                0.0
            } else {
                0.15 + 0.45 * fx / f64::from(W)
            };
            let h = nuclei
                .iter()
                .map(|&(cx, cy, r)| {
                    let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt() / r;
                    if d < 1.0 {
                        0.9 * (1.0 - d * d * 0.5)
                    } else {
                        0.0
                    }
                })
                .fold(0.0, f64::max);
            let od = stain::vec_mat([h, e, 0.0], &s.vectors);
            let c = od.map(|v| (10f64.powf(-v) * 255.0).round().clamp(0.0, 255.0) as u8);
            data.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    Raster {
        width: W,
        height: H,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::Rgba,
        planes: vec![RasterPlane {
            width: W,
            height: H,
            stride: W as usize * 4,
            data,
            h_div: 1,
            v_div: 1,
        }],
        truncated: false,
    }
}

struct Case {
    name: &'static str,
    visible: [bool; 3],
    mode: Mode,
    scale: f32,
    rtl: bool,
}

#[test]
fn stain_channels_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-stains",
    );
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, W, H, None);
    renderer.upload_overview(&h.gpu, &mut img, &field());
    let set = StainSet::He;
    let stains = Stains::published(set);
    let d = stain::display(&stains, None).expect("invertible");
    let cases = [
        Case {
            name: "stains-hematoxylin-light-1x",
            visible: [true, false, false],
            mode: Mode::Intensity,
            scale: 1.0,
            rtl: false,
        },
        Case {
            name: "stains-both-od-1.5x",
            visible: [true, true, false],
            mode: Mode::Od,
            scale: 1.5,
            rtl: false,
        },
        Case {
            name: "stains-eosin-light-he-1x",
            visible: [false, true, false],
            mode: Mode::Intensity,
            scale: 1.0,
            rtl: true,
        },
        Case {
            name: "stains-hematoxylin-od-he-1.5x",
            visible: [true, false, false],
            mode: Mode::Od,
            scale: 1.5,
            rtl: true,
        },
    ];
    let pal = Palette::dark();
    for c in cases {
        let lang = if c.rtl {
            specular_i18n::langid!("he")
        } else {
            specular_i18n::langid!("en-US")
        };
        let opts = RenderOptions {
            stain: Some(view_of(&d, c.visible, c.mode)),
            ..RenderOptions::default()
        };
        let (shot, tree) = specular_i18n::I18n::with_thread_language(lang, || {
            let v = legend_view(set, &stains, c.visible, c.mode, false);
            let mut drawn = None;
            let shot = h.render_gpu(
                SIZE,
                c.scale,
                |gpu, enc, target, size| {
                    let view = View::fixed(
                        (W, H),
                        Orientation::NORMAL,
                        PxRect {
                            x: 0.0,
                            y: 0.0,
                            w: size.0 as f32,
                            h: size.1 as f32,
                        },
                        size.0 as f32 / W as f32,
                        (W as f32 / 2.0, H as f32 / 2.0),
                    );
                    renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
                },
                |dl, text| {
                    let area = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
                    drawn = Some(draw_legend(dl, text, &pal, &v, area, c.rtl, 1.0));
                },
            );
            let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
            legend_a11y(&mut tree, &v, &drawn.expect("drawn"), 1.0);
            (shot, tree)
        });
        goldens.check(c.name, &shot);
        super::super::a11y_audit::expect_clean(c.name, &tree, c.rtl);
    }
    goldens.finish();
}

/// The legend's rows are where clicks toggle them, and mirror right to left.
#[test]
fn legend_rows_sit_in_the_panel_and_mirror() {
    let Some(mut h) = Harness::new() else { return };
    let s = Stains::published(StainSet::HeDab);
    let v = legend_view(
        StainSet::HeDab,
        &s,
        [true, false, true],
        Mode::Intensity,
        false,
    );
    let pal = Palette::dark();
    let area = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
    let mut out = Vec::new();
    h.render(
        SIZE,
        1.0,
        |_| {},
        |dl, text| {
            out.push(draw_legend(dl, text, &pal, &v, area, false, 1.0));
            out.push(draw_legend(dl, text, &pal, &v, area, true, 1.0));
        },
    );
    let (l, r) = (&out[0], &out[1]);
    assert_eq!(l.rows.len(), 3);
    for row in &l.rows {
        assert!(row.x >= l.panel.x && row.right() <= l.panel.right() + 0.01);
        assert!(row.y >= l.panel.y && row.bottom() <= l.panel.bottom());
    }
    for (a, b) in l.rows.iter().zip(&r.rows) {
        assert!((a.x - (SIZE.0 - b.right())).abs() < 0.01);
    }
    assert!(l.panel.right() <= SIZE.0 && l.panel.bottom() <= SIZE.1);
}
