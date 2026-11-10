//! Screenshot test of a two-page spread read right to left (manga): two synthetic comic
//! pages side by side on the GPU, the earlier page on the right, scaled to the taller, at 1×
//! and 1.5×, and fitted to the window's height. Rendered offscreen on lavapipe and compared
//! with the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular reading::shots`, then review and
//! commit the rewritten PNGs.

use specular_core::{PixelLayout, Raster, RasterPlane};
use specular_render::{GpuImageCache, ImageRenderer, RenderOptions};
use specular_shots::{Goldens, Harness};

use super::*;

const SIZE: (f32, f32) = (420.0, 300.0);

/// A comic page `w` × `h`: paper, a border, `n` black panels down the page and a tint.
fn page(w: u32, h: u32, n: u32, tint: [u8; 3]) -> Raster {
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    let m = w / 12;
    let gap = h / 24;
    let panel_h = (h - 2 * m - (n - 1) * gap) / n;
    for y in 0..h {
        for x in 0..w {
            let inside = x >= m && x < w - m && y >= m && y < h - m;
            let (in_panel, edge) = if inside {
                let yy = y - m;
                let k = yy / (panel_h + gap);
                let off = yy % (panel_h + gap);
                let in_p = k < n && off < panel_h;
                let edge = in_p && (off < 3 || off + 3 >= panel_h || x < m + 3 || x + 3 >= w - m);
                (in_p, edge)
            } else {
                (false, false)
            };
            let px = if edge {
                [20, 20, 20, 255]
            } else if in_panel {
                [tint[0], tint[1], tint[2], 255]
            } else {
                [250, 248, 240, 255]
            };
            data.extend_from_slice(&px);
        }
    }
    Raster {
        width: w,
        height: h,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::Rgba,
        planes: vec![RasterPlane {
            width: w,
            height: h,
            stride: w as usize * 4,
            data,
            h_div: 1,
            v_div: 1,
        }],
        truncated: false,
    }
}

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    scale: f32,
    rtl: bool,
    fit_width: bool,
}

fn shoot(h: &mut Harness, goldens: &mut Goldens, c: Case) {
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut cache = GpuImageCache::default();
    // Page 2 (three panels, blue) and page 3 (two panels, a smaller scan, orange).
    let (a, b) = (
        page(200, 300, 3, [150, 190, 230]),
        page(120, 180, 2, [240, 190, 140]),
    );
    for (id, r) in [(2u64, &a), (3u64, &b)] {
        let mut img = renderer.create_image(&h.gpu, id, r.width, r.height, None);
        renderer.upload_overview(&h.gpu, &mut img, r);
        cache.insert(img);
    }
    let pages = [
        Page {
            id: 2,
            orient: Orientation::NORMAL,
            stored: (200, 300),
            display: (200, 300),
        },
        Page {
            id: 3,
            orient: Orientation::NORMAL,
            stored: (120, 180),
            display: (120, 180),
        },
    ];
    let size_px = (
        (SIZE.0 * c.scale).round() as u32,
        (SIZE.1 * c.scale).round() as u32,
    );
    let (w, hh) = composite_size(&pages);
    let mut view = View::new();
    view.set_viewport(specular_render::PxRect {
        x: 0.0,
        y: 0.0,
        w: size_px.0 as f32,
        h: size_px.1 as f32,
    });
    view.upscale_small = true;
    view.set_image(w, hh, Orientation::NORMAL);
    if c.fit_width {
        view.zoom_fit_side(true, true);
    }
    let views = page_views(&pages, c.rtl, &view);
    let _ = h
        .gpu
        .device
        .poll(specular_gfx::wgpu::PollType::wait_indefinitely());
    let shot = h.render_gpu(
        SIZE,
        c.scale,
        |gpu, enc, target, size| {
            let opts = RenderOptions::default();
            render_spread(
                &mut renderer,
                gpu,
                enc,
                target,
                size,
                &mut cache,
                &views,
                &opts,
            );
        },
        |_dl, _text| {},
    );
    goldens.check(&format!("{}-{}x", c.name, c.scale), &shot);
}

#[test]
fn spreads_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-reading",
    );
    let base = Case {
        name: "",
        scale: 1.0,
        rtl: true,
        fit_width: false,
    };
    for c in [
        Case {
            name: "spread-rtl",
            ..base
        },
        Case {
            name: "spread-rtl",
            scale: 1.5,
            ..base
        },
        Case {
            name: "spread-ltr",
            rtl: false,
            ..base
        },
        Case {
            name: "spread-rtl-fit-width",
            fit_width: true,
            ..base
        },
    ] {
        shoot(&mut h, &mut goldens, c);
    }
    goldens.finish();
}
