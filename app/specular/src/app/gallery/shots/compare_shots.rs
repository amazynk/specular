use super::*;
use specular_render::Place;

/// Compare with the red channel on: three images rendered through the viewer's image
/// renderer into the compare cells, each caption naming the view.
#[test]
fn compare_channel_view_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let dims = [(120, 80), (96, 96), (80, 120)];
    let images: Vec<_> = dims
        .iter()
        .enumerate()
        .map(|(k, &(w, hh))| {
            let mut img = renderer.create_image(&h.gpu, k as u64 + 1, w, hh, None);
            renderer.upload_overview(&h.gpu, &mut img, &pattern(k, w, hh));
            img
        })
        .collect();
    let pal = Palette::dark();
    for (scale, suffix) in [(1.0, "1x"), (1.5, "1.5x")] {
        let mut g = gallery();
        g.filter_open = false;
        let paths: Vec<PathBuf> = g.tile_paths().take(3).cloned().collect();
        g.focus = paths.first().cloned();
        g.compare = Some(paths.clone());
        let cells = compare_layout(SIZE.0, SIZE.1, paths.len(), g.quick_caption_h(), 1, 1.0);
        let panes = paths
            .iter()
            .zip(&cells)
            .enumerate()
            .map(|(k, (p, (_, area)))| {
                let mut pane = QuickPane::new(
                    p.clone(),
                    k as u64 + 1,
                    dims[k],
                    Orientation::NORMAL,
                    *area,
                    scale,
                );
                // As `App::quick_pane` sets them over no backdrop.
                pane.channel = ChannelView::Red;
                pane.opts = RenderOptions {
                    hq: true,
                    canvas: [0.0; 4],
                    checker_a: [0.0; 4],
                    checker_b: [0.0; 4],
                    checker_cell: 1.0e6,
                    channel: ChannelView::Red,
                    ..RenderOptions::default()
                };
                pane
            })
            .collect();
        let plan = QuickPlan {
            panes,
            diffs: Vec::new(),
        };
        g.prepare_panes(&h.gpu, &mut h.ui, Some(&plan));
        let mut enc = h.gpu.device.create_command_encoder(&Default::default());
        for ((pane, t), img) in plan.panes.iter().zip(&g.quick_targets).zip(&images) {
            render_pane(&mut renderer, &h.gpu, &mut enc, t, img, pane);
        }
        h.gpu.queue.submit([enc.finish()]);
        let f = Frame {
            pal: &pal,
            now: 1.0,
            reduce_motion: true,
            size: SIZE,
            scale,
            rtl: false,
            text_scale: 1.0,
            backdrop: Backdrop::None,
        };
        // Each caption names the view.
        let e = g.entries[g.by_path[paths[1].as_path()]].clone();
        assert!(g.quick_caption(&e).ends_with("Red channel"));
        assert_eq!(g.channel_shown(), ChannelView::Red);
        let shot = h.render(
            SIZE,
            scale,
            |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas),
            |dl, text| {
                g.draw(dl, text, &f);
            },
        );
        goldens.check(&format!("gallery-compare-red-{suffix}"), &shot);
        g.prepare_panes(&h.gpu, &mut h.ui, None);
        assert!(g.quick_targets.is_empty() && g.quick_tex.is_empty());
    }
    goldens.finish();
}

/// Compare's overlay (the second image half over the first) and difference view (the
/// absolute error of the second and a smaller third against the first, with their metrics
/// from the GPU reduction and the note on the different size), with the toolbar of each.
#[test]
fn compare_overlay_and_difference_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let dims = [(120, 80), (120, 80), (100, 70)];
    let mut images = GpuImageCache::default();
    for (k, &(w, hh)) in dims.iter().enumerate() {
        let mut img = renderer.create_image(&h.gpu, k as u64 + 1, w, hh, None);
        renderer.upload_overview(&h.gpu, &mut img, &burst(k as u32, w, hh));
        images.insert(img);
    }
    let pal = Palette::dark();
    let sizes: Vec<Option<(f32, f32)>> = dims
        .iter()
        .map(|&(w, hh)| Some((w as f32, hh as f32)))
        .collect();
    let frame = super::compare::frame_of(&sizes);
    let shots = [
        (CmpMode::Overlay, 1.0, "gallery-compare-overlay-1x"),
        (CmpMode::Diff, 1.0, "gallery-compare-diff-1x"),
        (CmpMode::Diff, 1.5, "gallery-compare-diff-1.5x"),
    ];
    for (mode, scale, name) in shots {
        let mut g = gallery();
        g.filter_open = false;
        let paths: Vec<PathBuf> = g.tile_paths().take(3).cloned().collect();
        g.focus = paths.first().cloned();
        g.compare = Some(paths.clone());
        g.cmp.mode = mode;
        g.cmp.sizes = sizes.clone();
        g.cmp.comparer = Some(Comparer::new(&h.gpu));
        let cap = g.quick_caption_h();
        let n = if mode == CmpMode::Overlay { 1 } else { 2 };
        let cells = compare_layout(SIZE.0, SIZE.1, n, cap, g.cmp.caption_lines(), 1.0);
        let shown: Vec<(usize, Rect)> = match mode {
            CmpMode::Overlay => vec![(0, cells[0].1), (1, cells[0].1)],
            _ => vec![(0, cells[0].1), (1, cells[0].1), (2, cells[1].1)],
        };
        let mut idx = vec![None; 3];
        let panes: Vec<QuickPane> = shown
            .iter()
            .enumerate()
            .map(|(j, &(k, area))| {
                idx[k] = Some(j);
                let mut pane = QuickPane::new(
                    paths[k].clone(),
                    k as u64 + 1,
                    dims[k],
                    Orientation::NORMAL,
                    area,
                    scale,
                )
                .framed(frame, area, scale);
                pane.opts = RenderOptions {
                    hq: true,
                    canvas: [0.0; 4],
                    checker_a: [0.0; 4],
                    checker_b: [0.0; 4],
                    checker_cell: 1.0e6,
                    ..RenderOptions::default()
                };
                pane
            })
            .collect();
        let kind = specular_inspect::DiffKind::Absolute;
        let (diffs, pairs) = if mode == CmpMode::Diff {
            let opts = super::compare::DiffOptions {
                full_scale: kind.full_scale(specular_inspect::DEFAULT_SCALE),
                kind,
                channel: ChannelView::Normal,
                space: specular_inspect::MetricSpace::Linear,
                whole: false,
            };
            diff_panes(
                &panes,
                &idx,
                &sizes,
                &[Place::IDENTITY; 3],
                &[false; 3],
                &opts,
            )
        } else {
            (Vec::new(), Vec::new())
        };
        let plan = QuickPlan { panes, diffs };
        g.prepare_panes(&h.gpu, &mut h.ui, Some(&plan));
        let mut enc = h.gpu.device.create_command_encoder(&Default::default());
        for (pane, t) in plan.panes.iter().zip(&g.quick_targets) {
            let img = images.get(pane.metric_image().id).expect("uploaded");
            render_pane(&mut renderer, &h.gpu, &mut enc, t, img, pane);
        }
        let drawn = vec![true; plan.panes.len()];
        let c = g.cmp.comparer.as_ref().expect("comparer");
        render_diffs(
            c,
            &h.gpu,
            &mut enc,
            &plan,
            &g.quick_targets,
            &g.diff_targets,
            &drawn,
        );
        h.gpu.queue.submit([enc.finish()]);
        if mode == CmpMode::Diff {
            // The metrics, as `App::tick_compare` takes them.
            assert_eq!(pairs.len(), 2);
            let mut c = g.cmp.comparer.take().expect("comparer");
            let mut enc = h.gpu.device.create_command_encoder(&Default::default());
            let list: Vec<_> = pairs.iter().map(|p| p.1).collect();
            assert!(c.request(&h.gpu, &mut renderer, &mut images, &mut enc, &list));
            h.gpu.queue.submit([enc.finish()]);
            c.after_submit();
            let r = loop {
                if let Some(r) = c.poll(&h.gpu) {
                    break r;
                }
                let _ = h
                    .gpu
                    .device
                    .poll(specular_gfx::wgpu::PollType::wait_indefinitely());
            };
            let keys: Vec<usize> = pairs.iter().map(|p| p.0).collect();
            assert!(
                r.pairs
                    .iter()
                    .all(|m| m.as_ref().is_some_and(|m| m.stats.mae > 0.0))
            );
            g.cmp.set_metrics(&keys, r);
            g.cmp.comparer = Some(c);
        } else {
            g.cmp.blend = 0.5;
        }
        let f = Frame {
            pal: &pal,
            now: 1.0,
            reduce_motion: true,
            size: SIZE,
            scale,
            rtl: false,
            text_scale: 1.0,
            backdrop: Backdrop::None,
        };
        let shot = h.render(
            SIZE,
            scale,
            |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas),
            |dl, text| {
                g.draw(dl, text, &f);
            },
        );
        goldens.check(name, &shot);
        g.prepare_panes(&h.gpu, &mut h.ui, None);
        assert!(g.diff_targets.is_empty() && g.diff_tex.is_empty());
    }
    goldens.finish();
}

/// One two-image compare shot: what it shows and how.
struct TwoUp {
    name: &'static str,
    mode: CmpMode,
    kind: specular_inspect::DiffKind,
    align: super::compare::AlignMode,
    across: bool,
    space: specular_inspect::MetricSpace,
    whole: bool,
    probe: bool,
    scale: f32,
    rtl: bool,
}

impl TwoUp {
    fn new(name: &'static str, mode: CmpMode, scale: f32) -> TwoUp {
        TwoUp {
            name,
            mode,
            kind: specular_inspect::DiffKind::Absolute,
            align: super::compare::AlignMode::TopLeft,
            across: false,
            space: specular_inspect::MetricSpace::Linear,
            whole: false,
            probe: false,
            scale,
            rtl: false,
        }
    }
}

/// Draws compare of two images (`dims`, uploaded as `images`) as `t` says: the panes, the
/// differences and their metrics from the GPU, and the probe's readback.
fn two_up(
    h: &mut Harness,
    renderer: &mut ImageRenderer,
    images: &mut GpuImageCache,
    dims: &[(u32, u32)],
    t: &TwoUp,
) -> specular_shots::Shot {
    let pal = Palette::dark();
    let scale = t.scale;
    let mut g = gallery();
    g.filter_open = false;
    g.rtl = t.rtl;
    let paths: Vec<PathBuf> = g.tile_paths().take(dims.len()).cloned().collect();
    g.focus = paths.first().cloned();
    g.compare = Some(paths.clone());
    let sizes: Vec<Option<(f32, f32)>> = dims
        .iter()
        .map(|&(w, hh)| Some((w as f32, hh as f32)))
        .collect();
    g.cmp.mode = t.mode;
    g.cmp.kind = t.kind;
    g.cmp.align = t.align;
    g.cmp.split.across = t.across;
    g.cmp.space = t.space;
    g.cmp.whole = t.whole;
    g.cmp.sizes = sizes.clone();
    g.cmp.comparer = Some(Comparer::new(&h.gpu));
    let frame = g.cmp.place_images();
    let places = g.cmp.places.clone();
    let cap = g.quick_caption_h();
    let cells = compare_layout(SIZE.0, SIZE.1, 1, cap, g.cmp.caption_lines(), 1.0);
    let area = cells[0].1;
    let mut idx = vec![None; dims.len()];
    let panes: Vec<QuickPane> = (0..dims.len())
        .map(|k| {
            idx[k] = Some(k);
            let mut pane = QuickPane::new(
                paths[k].clone(),
                k as u64 + 1,
                dims[k],
                Orientation::NORMAL,
                area,
                scale,
            )
            .framed(frame, area, scale);
            pane.place = places[k];
            pane.opts = RenderOptions {
                hq: true,
                canvas: [0.0; 4],
                checker_a: [0.0; 4],
                checker_b: [0.0; 4],
                checker_cell: 1.0e6,
                ..RenderOptions::default()
            };
            pane
        })
        .collect();
    let (diffs, pairs) = if t.mode == CmpMode::Diff {
        let opts = super::compare::DiffOptions {
            full_scale: t.kind.full_scale(specular_inspect::DEFAULT_SCALE),
            kind: t.kind,
            channel: ChannelView::Normal,
            space: t.space,
            whole: t.whole,
        };
        diff_panes(&panes, &idx, &sizes, &places, &[false, false], &opts)
    } else {
        (Vec::new(), Vec::new())
    };
    let plan = QuickPlan { panes, diffs };
    g.prepare_panes(&h.gpu, &mut h.ui, Some(&plan));
    let mut enc = h.gpu.device.create_command_encoder(&Default::default());
    for (pane, tg) in plan.panes.iter().zip(&g.quick_targets) {
        let img = images.get(pane.metric_image().id).expect("uploaded");
        render_pane(renderer, &h.gpu, &mut enc, tg, img, pane);
    }
    let drawn = vec![true; plan.panes.len()];
    let c = g.cmp.comparer.as_ref().expect("comparer");
    render_diffs(
        c,
        &h.gpu,
        &mut enc,
        &plan,
        &g.quick_targets,
        &g.diff_targets,
        &drawn,
    );
    h.gpu.queue.submit([enc.finish()]);
    let mut c = g.cmp.comparer.take().expect("comparer");
    let wait = |h: &Harness| {
        let _ = h
            .gpu
            .device
            .poll(specular_gfx::wgpu::PollType::wait_indefinitely());
    };
    if t.mode == CmpMode::Diff {
        let mut enc = h.gpu.device.create_command_encoder(&Default::default());
        let list: Vec<_> = pairs.iter().map(|p| p.1).collect();
        assert!(
            c.request(&h.gpu, renderer, images, &mut enc, &list),
            "{}",
            t.name
        );
        h.gpu.queue.submit([enc.finish()]);
        c.after_submit();
        let r = loop {
            if let Some(r) = c.poll(&h.gpu) {
                break r;
            }
            wait(h);
        };
        let keys: Vec<usize> = pairs.iter().map(|p| p.0).collect();
        assert!(
            r.pairs
                .iter()
                .all(|m| m.as_ref().is_some_and(|m| m.ssim.is_some()))
        );
        g.cmp.set_metrics(&keys, r);
    }
    if t.probe {
        let geo: Vec<(usize, QuickPane)> = plan.panes.iter().cloned().enumerate().collect();
        let r = plan.panes[0].placed().0;
        let want = g
            .cmp
            .probe_shot(geo, (r.x + r.w * 0.4, r.y + r.h * 0.5), scale);
        let texels: Vec<(&specular_gfx::wgpu::Texture, (u32, u32))> = want
            .iter()
            .flatten()
            .map(|(pane, at)| (g.quick_targets[*pane].texture(), *at))
            .collect();
        let mut enc = h.gpu.device.create_command_encoder(&Default::default());
        assert!(c.request_probe(&mut enc, &texels));
        h.gpu.queue.submit([enc.finish()]);
        c.probe_after_submit();
        let got = loop {
            if let Some(v) = c.poll_probe(&h.gpu) {
                break v;
            }
            wait(h);
        };
        g.cmp
            .probe_read([want[0].is_some(), want[1].is_some()], &got);
    }
    g.cmp.comparer = Some(c);
    let f = Frame {
        pal: &pal,
        now: 1.0,
        reduce_motion: true,
        size: SIZE,
        scale,
        rtl: t.rtl,
        text_scale: 1.0,
        backdrop: Backdrop::None,
    };
    let shot = h.render(
        SIZE,
        scale,
        |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas),
        |dl, text| {
            g.draw(dl, text, &f);
        },
    );
    // The split's divider is a slider for screen readers, the probe a readout.
    let mut tree = super::super::super::a11y_audit::window(SIZE, 1.0);
    g.a11y(&mut tree, 1.0);
    super::super::super::a11y_audit::expect_clean(t.name, &tree, t.rtl);
    if t.mode == CmpMode::Split {
        assert!(
            tree.nodes.iter().any(
                |(_, n)| n.role() == specular_widgets::accesskit::Role::Slider
                    && n.numeric_value() == Some(50.0)
            ),
            "{}: the divider's slider",
            t.name
        );
    }
    g.prepare_panes(&h.gpu, &mut h.ui, None);
    shot
}

/// Two images compared: the split (down, across, and right to left in Hebrew), the signed
/// difference, SSIM and ΔE2000 of the whole image in sRGB, a half-size export scaled to the
/// original, and the pixel probe over the overlay.
#[test]
fn compare_two_images_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut images = GpuImageCache::default();
    let dims = [(120, 80), (120, 80), (60, 40)];
    for (k, &(w, hh)) in dims.iter().enumerate() {
        let mut img = renderer.create_image(&h.gpu, k as u64 + 1, w, hh, None);
        // The third: the first at half its size.
        let r = if k == 2 {
            burst(0, w, hh)
        } else {
            burst(k as u32, w, hh)
        };
        renderer.upload_overview(&h.gpu, &mut img, &r);
        images.insert(img);
    }
    use super::compare::AlignMode;
    use specular_inspect::{DiffKind, MetricSpace};
    let mut shots = Vec::new();
    for (scale, sfx) in [(1.0, "1x"), (1.5, "1.5x")] {
        let name =
            |base: &str| -> &'static str { Box::leak(format!("{base}-{sfx}").into_boxed_str()) };
        shots.push(TwoUp::new(
            name("gallery-compare-split"),
            CmpMode::Split,
            scale,
        ));
        shots.push(TwoUp {
            across: true,
            ..TwoUp::new(name("gallery-compare-split-h"), CmpMode::Split, scale)
        });
        shots.push(TwoUp {
            kind: DiffKind::Signed,
            ..TwoUp::new(name("gallery-compare-diff-signed"), CmpMode::Diff, scale)
        });
        shots.push(TwoUp {
            probe: true,
            ..TwoUp::new(name("gallery-compare-probe"), CmpMode::Overlay, scale)
        });
        shots.push(TwoUp {
            space: MetricSpace::Srgb,
            whole: true,
            ..TwoUp::new(
                name("gallery-compare-metrics-ssim-de"),
                CmpMode::Diff,
                scale,
            )
        });
    }
    shots.push(TwoUp {
        align: AlignMode::ScaleToA,
        ..TwoUp::new("gallery-compare-aligned-1x", CmpMode::Diff, 1.0)
    });
    shots.push(TwoUp {
        rtl: true,
        ..TwoUp::new("gallery-compare-split-he-rtl", CmpMode::Split, 1.0)
    });
    for t in &shots {
        // The aligned shot compares the half-size third with the first.
        let d: Vec<(u32, u32)> = if t.align == AlignMode::ScaleToA {
            vec![dims[0], dims[2]]
        } else {
            dims[..2].to_vec()
        };
        let shot = if t.rtl {
            specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
                two_up(&mut h, &mut renderer, &mut images, &d, t)
            })
        } else if t.align == AlignMode::ScaleToA {
            // Ids 1 and 3: give the third image the second's place.
            let mut imgs = GpuImageCache::default();
            for (k, &(w, hh)) in d.iter().enumerate() {
                let mut img = renderer.create_image(&h.gpu, k as u64 + 1, w, hh, None);
                renderer.upload_overview(&h.gpu, &mut img, &burst(0, w, hh));
                imgs.insert(img);
            }
            two_up(&mut h, &mut renderer, &mut imgs, &d, t)
        } else {
            two_up(&mut h, &mut renderer, &mut images, &d, t)
        };
        goldens.check(t.name, &shot);
    }
    goldens.finish();
}
