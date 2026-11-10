//! Screenshot tests of relighting: the synthetic order-3 HSH fixture of specular-rti relit
//! on the GPU by the image renderer under several lights and in every mode, with the light
//! widget, at 1× and 1.5×, and right to left in Hebrew. Rendered offscreen on lavapipe and
//! compared with the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular relight::shots`, then review and
//! commit the rewritten PNGs.

use specular_core::Orientation;
use specular_render::{ImageRenderer, PxRect, RenderOptions, View};
use specular_shots::{Goldens, Harness};

use super::*;

const SIZE: (f32, f32) = (360.0, 260.0);

const FIXTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../crates/formats/specular-rti/tests/data/hsh3.rti"
));

/// The view of the whole image filling most of a `size` (physical px) target.
fn view_of(size: (u32, u32), w: u32, h: u32) -> View {
    View::fixed(
        (w, h),
        Orientation::NORMAL,
        PxRect {
            x: 0.0,
            y: 0.0,
            w: size.0 as f32,
            h: size.1 as f32,
        },
        size.0 as f32 / w as f32 * 0.9,
        (w as f32 / 2.0, h as f32 / 2.0),
    )
}

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    /// The light over this point of the unit disk.
    disk: (f32, f32),
    mode: Mode,
    rtl: bool,
    scale: f32,
}

fn shoot(h: &mut Harness, goldens: &mut Goldens, rti: &Rti, c: &Case) {
    let look = Look {
        light: Look::light_from_disk(c.disk.0, c.disk.1),
        mode: c.mode,
        ..Default::default()
    };
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, rti.width, rti.height, None);
    assert!(renderer.set_relight(&h.gpu, &mut img, rti));
    assert!(renderer.relight(&h.gpu, &mut img, &rti.prepare(&look)));
    let size_px = (
        (SIZE.0 * c.scale).round() as u32,
        (SIZE.1 * c.scale).round() as u32,
    );
    let view = view_of(size_px, rti.width, rti.height);
    let rtl = c.rtl;
    let shot = h.render_gpu(
        SIZE,
        c.scale,
        |gpu, enc, target, size| {
            let opts = RenderOptions::default();
            renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
        },
        |dl, _text| {
            let area = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
            draw_widget(dl, widget_rect(area, rtl), &look);
        },
    );
    goldens.check(&format!("{}-{}x", c.name, c.scale), &shot);
    let r = widget_rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), rtl);
    super::super::a11y_audit::audit_screen(c.name, SIZE, rtl, |t| a11y(t, &look, r, 1.0));
}

#[test]
fn relit_views_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-relight",
    );
    let rti = Rti::read(FIXTURE).unwrap();
    let base = Case {
        name: "",
        disk: (0.0, 0.0),
        mode: Mode::Default,
        rtl: false,
        scale: 1.0,
    };
    let cases = [
        Case {
            name: "relight-above",
            ..base
        },
        Case {
            name: "relight-raking-left",
            disk: (-0.9, 0.3),
            scale: 1.5,
            ..base
        },
        Case {
            name: "relight-specular",
            disk: (0.4, 0.5),
            mode: Mode::Specular,
            ..base
        },
        Case {
            name: "relight-normals",
            mode: Mode::Normals,
            ..base
        },
        Case {
            name: "relight-diffuse-gain",
            disk: (0.5, -0.4),
            mode: Mode::DiffuseGain,
            ..base
        },
    ];
    for c in &cases {
        shoot(&mut h, &mut goldens, &rti, c);
    }
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        let c = Case {
            name: "relight-raking-he",
            disk: (0.7, 0.2),
            rtl: true,
            ..base
        };
        shoot(&mut h, &mut goldens, &rti, &c);
    });
    goldens.finish();
}
