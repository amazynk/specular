//! Screenshot tests of the gallery at 1×, 1.5× and 2×: the grid
//! with the filter bar open, in dark and light; and compare in a channel view (the red channel
//! of three synthetic images through the image renderer, at 1× and 1.5×); and the
//! similar-shots review zoomed to 100 %, in the normal view and the red channel (1× and 1.5×).
//! Rendered offscreen on lavapipe and compared with the PNGs in `app/specular/tests/shots/`; see `specular-shots` for the tolerance and what a
//! failure leaves behind.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular gallery::shots`, then review and
//! commit the rewritten PNGs.
//!
//! The tiles are synthetic (no files are read): each is the placeholder in its own average
//! colour, as before its thumbnail arrives, with its caption. The UI language is process-wide
//! and these tests share their process with the app's other unit tests, so the Hebrew shots
//! switch it for their own thread only ([`specular_i18n::I18n::with_thread_language`]).

use super::atlas::Atlas;
use super::compare::{CmpMode, compare_layout, diff_panes};
use super::draw::Frame;
use super::quick::{QuickPane, QuickPlan, render_diffs, render_pane};
use super::*;
use specular_core::{Orientation, PixelLayout, Raster, RasterPlane};
use specular_gfx::DrawList;
use specular_render::{Comparer, GpuImageCache, ImageRenderer, RenderOptions};
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;

/// Logical window size, as for the chrome's shots.
const SIZE: (f32, f32) = (720.0, 450.0);
const SCALES: [(f32, &str); 3] = [(1.0, "1x"), (1.5, "1.5x"), (2.0, "2x")];

/// A folder of 24 photos, 16 of them from the beach (which the filter keeps), in a few
/// aspect ratios and colours.
fn gallery() -> Gallery {
    let mut g = Gallery {
        active: true,
        filter_open: true,
        ..Gallery::default()
    };
    let colours: [u32; 6] = [0x3d78b8, 0x6aa84f, 0xe69138, 0xc27ba0, 0x45818e, 0xf1c232];
    let dims = [(4000, 3000), (3000, 4000), (4000, 2250), (3000, 3000)];
    let entries: Vec<Entry> = (0..24usize)
        .map(|i| {
            let name = if i % 3 == 2 {
                format!("forest-{:02}.jpg", i + 1)
            } else {
                format!("beach-{:02}.jpg", i + 1)
            };
            let path = PathBuf::from(format!("/shots/{name}"));
            let c = colours[i % colours.len()];
            g.averages.insert(
                path.as_path().into(),
                [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255],
            );
            Entry {
                path,
                name,
                kind: EntryKind::Image,
                len: 2_000_000 + i as u64 * 1000,
                modified: None,
                hidden: false,
                taken: None,
                dims: Some(dims[i % dims.len()]),
            }
        })
        .collect();
    g.add_entries(entries);
    g.filter.text = "beach".into();
    g.resort();
    g.rebuild();
    // An empty atlas: the tiles draw as placeholders in their average colour, with captions.
    g.atlas = Some(Atlas::new(4));
    g
}

#[test]
fn gallery_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    let t0 = Instant::now();
    let mut n = 0;
    for (name, dark) in [("gallery-dark", true), ("gallery-light", false)] {
        let pal = if dark {
            Palette::dark()
        } else {
            Palette::light()
        };
        for (scale, suffix) in SCALES {
            let mut g = gallery();
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
            // Frames until every caption is shaped (they are spread over frames by a time
            // budget), then the one that is compared.
            for _ in 0..20 {
                let mut warm = DrawList::new(scale);
                h.text.begin_frame(scale);
                g.draw(&mut warm, &mut h.text, &f);
                if !g.cap_more {
                    break;
                }
            }
            assert!(!g.cap_more, "captions still pending");
            let shot = h.render(
                SIZE,
                scale,
                |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas),
                |dl, text| {
                    g.draw(dl, text, &f);
                },
            );
            goldens.check(&format!("{name}-{suffix}"), &shot);
            let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
            g.a11y(&mut tree, 1.0);
            super::super::a11y_audit::expect_clean(&format!("{name}-{suffix}"), &tree, f.rtl);
            n += 1;
        }
    }
    eprintln!(
        "{n} gallery screenshots on {} in {:.2} s",
        h.gpu.info.name,
        t0.elapsed().as_secs_f64()
    );
    goldens.finish();
}

/// Archive tiles: mosaics of their first pages (four, two) with the extension badge, one whose
/// pages could not be listed, beside a folder's mosaic; in dark at 1× and 1.5×.
/// The bottom bar's free stretch (where downloads show) follows a resize in the same
/// frame: the chrome, drawn after the gallery, never places the line from the old width.
#[test]
fn bottom_bar_free_stretch_follows_a_resize_at_once() {
    let Some(mut h) = Harness::new() else { return };
    let pal = Palette::dark();
    let mut g = gallery();
    let mut frame_at = |g: &mut Gallery, w: f32| {
        let f = Frame {
            pal: &pal,
            now: 1.0,
            reduce_motion: true,
            size: (w, SIZE.1),
            scale: 1.0,
            rtl: false,
            text_scale: 1.0,
            backdrop: Backdrop::None,
        };
        let mut dl = DrawList::new(1.0);
        h.text.begin_frame(1.0);
        g.draw(&mut dl, &mut h.text, &f);
        g.status_free
    };
    let wide = frame_at(&mut g, 1100.0);
    let narrow = frame_at(&mut g, 800.0);
    assert!(wide.1 < 1100.0 && narrow.1 < 800.0, "{wide:?} {narrow:?}");
    // The controls on the right keep their widths: the stretch's end moves with the edge.
    assert!(
        (wide.1 - narrow.1 - 300.0).abs() < 0.5,
        "{wide:?} {narrow:?}"
    );
    assert_eq!(wide.0, narrow.0, "the count does not move");
    let back = frame_at(&mut g, 1100.0);
    assert_eq!(back, wide);
}

#[test]
fn archive_tiles_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    let pal = Palette::dark();
    let colours: [u32; 6] = [0x3d78b8, 0x6aa84f, 0xe69138, 0xc27ba0, 0x45818e, 0xf1c232];
    for (scale, suffix) in &SCALES[..2] {
        let mut g = Gallery {
            active: true,
            show_archives: true,
            ..Gallery::default()
        };
        let tiles: [(&str, EntryKind, usize); 4] = [
            ("Holiday", EntryKind::Folder, 4),
            ("comic.cbz", EntryKind::Archive, 4),
            ("manga.cb7", EntryKind::Archive, 2),
            ("broken.zip", EntryKind::Archive, 0),
        ];
        let mut entries = Vec::new();
        for (t, &(name, kind, pages)) in tiles.iter().enumerate() {
            let path = PathBuf::from(format!("/shots/{name}"));
            let covers: Vec<PathBuf> = (0..pages)
                .map(|k| {
                    let p = path.join(format!("{:03}.jpg", k + 1));
                    let c = colours[(t + k) % colours.len()];
                    g.averages.insert(
                        p.as_path().into(),
                        [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255],
                    );
                    p
                })
                .collect();
            g.covers.insert(path.clone(), covers);
            entries.push(Entry {
                path,
                name: name.into(),
                kind,
                len: 0,
                modified: None,
                hidden: false,
                taken: None,
                dims: None,
            });
        }
        g.add_entries(entries);
        g.resort();
        g.rebuild();
        g.atlas = Some(Atlas::new(4));
        let f = Frame {
            pal: &pal,
            now: 1.0,
            reduce_motion: true,
            size: SIZE,
            scale: *scale,
            rtl: false,
            text_scale: 1.0,
            backdrop: Backdrop::None,
        };
        for _ in 0..20 {
            let mut warm = DrawList::new(*scale);
            h.text.begin_frame(*scale);
            g.draw(&mut warm, &mut h.text, &f);
            if !g.cap_more {
                break;
            }
        }
        assert!(!g.cap_more, "captions still pending");
        let shot = h.render(
            SIZE,
            *scale,
            |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas),
            |dl, text| {
                g.draw(dl, text, &f);
            },
        );
        goldens.check(&format!("gallery-archives-{suffix}"), &shot);
        let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
        g.a11y(&mut tree, 1.0);
        super::super::a11y_audit::expect_clean(&format!("gallery-archives-{suffix}"), &tree, f.rtl);
    }
    goldens.finish();
}

/// The focused tile (selected, with another one) with the keyboard in the grid: the focus
/// ring; and with the keyboard in the filter field: drawn as the plain selection (1× and
/// 1.5×).
#[test]
fn gallery_focus_matches_goldens() {
    use super::filterbar::Stop;
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery-focus",
    );
    let pal = Palette::dark();
    for (name, grid) in [
        ("gallery-focus-grid-dark", true),
        ("gallery-focus-field-dark", false),
    ] {
        for (scale, suffix) in &SCALES[..2] {
            let scale = *scale;
            let mut g = gallery();
            g.go(Stop::Grid);
            g.set_focus(2);
            for i in [0, 2] {
                let p = g.entry_of(i).unwrap().path.clone();
                g.selected.insert(p);
            }
            if !grid {
                g.go(Stop::Field);
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
            for _ in 0..20 {
                let mut warm = DrawList::new(scale);
                h.text.begin_frame(scale);
                g.draw(&mut warm, &mut h.text, &f);
                if !g.cap_more {
                    break;
                }
            }
            assert!(!g.cap_more, "captions still pending");
            assert_eq!(g.grid_has_keyboard(), grid);
            let shot = h.render(
                SIZE,
                scale,
                |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas),
                |dl, text| {
                    g.draw(dl, text, &f);
                },
            );
            goldens.check(&format!("{name}-{suffix}"), &shot);
            let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
            g.a11y(&mut tree, 1.0);
            super::super::a11y_audit::expect_clean(&format!("{name}-{suffix}"), &tree, f.rtl);
        }
    }
    goldens.finish();
}

/// A synthetic straight-alpha RGBA image of `w` × `h`: `k` 0 a red ramp left to right over
/// falling green, 1 four bars (red, green, blue, white), 2 red across and green down.
fn pattern(k: usize, w: u32, h: u32) -> Raster {
    let mut data = Vec::with_capacity(w as usize * h as usize * 4);
    for y in 0..h {
        for x in 0..w {
            let fx = (x * 255 / (w - 1)) as u8;
            let fy = (y * 255 / (h - 1)) as u8;
            let px = match k {
                0 => [fx, 255 - fx, 64, 255],
                1 => [
                    [255, 0, 0, 255],
                    [0, 255, 0, 255],
                    [0, 0, 255, 255],
                    [255; 4],
                ][(x * 4 / w) as usize],
                _ => [fx, fy, 128, 255],
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

/// A synthetic "burst" frame of `w` × `h`: a gradient with fine detail that only shows at
/// 100 % (1 px lines, a 2 px checker), shifted by `k` px per frame as a handheld burst is.
fn burst(k: u32, w: u32, h: u32) -> Raster {
    let mut data = Vec::with_capacity(w as usize * h as usize * 4);
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = (x + k * 3, y + k);
            let g = [
                (60 + sy * 120 / h) as u8,
                (110 + sy * 80 / h) as u8,
                (200 - sy * 120 / h) as u8,
                255,
            ];
            let px = if (w / 4..w / 2).contains(&sx) && (h / 4..h * 3 / 4).contains(&sy) {
                if sx % 4 == 0 { [20, 20, 20, 255] } else { g }
            } else if (w / 2 + 20..w * 3 / 4).contains(&sx) && (h / 3..h * 2 / 3).contains(&sy) {
                if (sx / 2 + sy / 2) % 2 == 0 {
                    [250, 250, 250, 255]
                } else {
                    [200, 40, 60, 255]
                }
            } else {
                g
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

/// The burst fixture of the similar-shots tests: three 800 × 600 frames.
fn burst_fixture() -> (
    Vec<specular_similar::scan::Unit>,
    Vec<specular_similar::Item>,
) {
    use specular_similar::Item;
    use specular_similar::scan::Unit;
    let paths: Vec<PathBuf> = (1..=3)
        .map(|k| PathBuf::from(format!("/shots/burst-{k}.jpg")))
        .collect();
    let units = paths
        .iter()
        .map(|p| Unit {
            main: p.clone(),
            files: vec![p.clone()],
            sig_path: p.clone(),
        })
        .collect();
    let items = paths
        .iter()
        .map(|p| Item {
            name: p.file_name().unwrap().to_string_lossy().into_owned(),
            width: 800,
            height: 600,
            bit_depth: 8,
            bytes: 2_000_000,
            len: 2_000_000,
            ..Item::default()
        })
        .collect();
    (units, items)
}

/// The grid in Hebrew: the bottom bar mirrored, the size slider small on the right with its
/// fill from there to the knob.
#[test]
fn gallery_rtl_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    let pal = Palette::light();
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        for (scale, suffix) in [(1.0, "1x"), (1.5, "1.5x")] {
            let mut g = gallery();
            let f = Frame {
                pal: &pal,
                now: 1.0,
                reduce_motion: true,
                size: SIZE,
                scale,
                rtl: true,
                text_scale: 1.0,
                backdrop: Backdrop::None,
            };
            for _ in 0..20 {
                let mut warm = DrawList::new(scale);
                h.text.begin_frame(scale);
                g.draw(&mut warm, &mut h.text, &f);
                if !g.cap_more {
                    break;
                }
            }
            let shot = h.render(
                SIZE,
                scale,
                |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas),
                |dl, text| {
                    g.draw(dl, text, &f);
                },
            );
            goldens.check(&format!("gallery-he-light-{suffix}"), &shot);
            let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
            g.a11y(&mut tree, 1.0);
            super::super::a11y_audit::expect_clean(
                &format!("gallery-he-light-{suffix}"),
                &tree,
                f.rtl,
            );
        }
    });
    goldens.finish();
}

/// A folder where two files could not be read: one a decoder panicked on (the alert icon,
/// the same tile as a damaged file) and one too big for the free memory (the icon and
/// "Needs … of free RAM"); the tiles around them draw as usual. Dark at 1× and 1.5×.
#[test]
fn failed_tiles_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery",
    );
    let pal = Palette::dark();
    for (scale, suffix) in [(1.0, "1x"), (1.5, "1.5x")] {
        let mut g = gallery();
        g.filter_open = false;
        g.filter.text.clear();
        g.resort();
        g.rebuild();
        let paths: Vec<PathBuf> = g.entries.iter().map(|e| e.path.clone()).collect();
        g.failed.insert(
            paths[1].clone(),
            std::sync::Arc::new(specular_decode::DecodeError::Truncated("x".into())),
        );
        g.failed.insert(
            paths[4].clone(),
            std::sync::Arc::new(specular_decode::DecodeError::NeedsMemory(
                specular_core::NeedsMemory {
                    needed: 24 << 30,
                    free: 9 << 30,
                },
            )),
        );
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
        for _ in 0..20 {
            let mut warm = DrawList::new(scale);
            h.text.begin_frame(scale);
            g.draw(&mut warm, &mut h.text, &f);
            if !g.cap_more {
                break;
            }
        }
        let shot = h.render(
            SIZE,
            scale,
            |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas),
            |dl, text| {
                g.draw(dl, text, &f);
            },
        );
        goldens.check(&format!("gallery-failed-tiles-{suffix}"), &shot);
        let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
        g.a11y(&mut tree, 1.0);
        super::super::a11y_audit::expect_clean(
            &format!("gallery-failed-tiles-{suffix}"),
            &tree,
            f.rtl,
        );
    }
    goldens.finish();
}

/// Where the map fixture's photos were taken: a few in each of some cities (several close
/// together, so they cluster), and some with no position.
const MAP_PLACES: [(f64, f64, usize); 6] = [
    (38.7223, -9.1393, 8),   // Lisbon
    (41.1579, -8.6291, 3),   // Porto
    (35.6762, 139.6503, 5),  // Tokyo
    (40.7128, -74.0060, 1),  // New York
    (-33.8688, 151.2093, 2), // Sydney
    (-33.9249, 18.4241, 1),  // Cape Town
];

/// The folder of [`gallery`] unfiltered, its photos placed as [`MAP_PLACES`] (spread a little
/// round each city), and the map open.
fn map_gallery() -> Gallery {
    let mut g = gallery();
    g.filter_open = false;
    g.filter.text.clear();
    g.resort();
    g.rebuild();
    let paths: Vec<PathBuf> = g.entries.iter().map(|e| e.path.clone()).collect();
    let mut k = 0;
    for &(lat, lon, n) in &MAP_PLACES {
        for j in 0..n {
            let Some(p) = paths.get(k) else { break };
            let a = j as f64 * 2.399_963;
            let r = 0.004 * (j as f64).sqrt();
            let meta = Meta {
                width: 4000,
                height: 3000,
                position: Some(((lat + r * a.sin()) as f32, (lon + r * a.cos()) as f32)),
                ..Meta::default()
            };
            g.metas.insert(p.as_path().into(), Arc::new(meta));
            k += 1;
        }
    }
    g.map.open = true;
    g
}

/// Draws the map until its captions are shaped, then renders its base map on the GPU, as a
/// frame of the app does (draw, prepare, render, then the UI pass).
fn map_frame(h: &mut Harness, g: &mut Gallery, f: &Frame) {
    for _ in 0..20 {
        let mut warm = DrawList::new(f.scale);
        h.text.begin_frame(f.scale);
        g.draw(&mut warm, &mut h.text, f);
        if !g.cap_more {
            break;
        }
    }
    g.prepare_map(&h.gpu, &mut h.ui, f.scale, f.size);
    let mut enc = h.gpu.device.create_command_encoder(&Default::default());
    g.render_map(&h.gpu, &mut enc);
    h.gpu.queue.submit([enc.finish()]);
}

/// A card's new shots for the import view: RAW+JPEG pairs, JPEGs, a RAW and videos, over two
/// days, from one camera.
fn import_shots() -> (Gallery, Vec<specular_import::Shot>) {
    use specular_import::{Media, MediaKind, Shot};
    let mut g = Gallery {
        active: true,
        ..Gallery::default()
    };
    g.atlas = Some(Atlas::new(4));
    let colours: [u32; 6] = [0x3d78b8, 0x6aa84f, 0xe69138, 0xc27ba0, 0x45818e, 0xf1c232];
    let media = |name: &str, kind: MediaKind, size: u64| Media {
        path: PathBuf::from(format!("/media/me/EOS_DIGITAL/DCIM/100CANON/{name}")),
        size,
        mtime: std::time::SystemTime::UNIX_EPOCH,
        kind,
        sidecars: Vec::new(),
        key: [0; 32],
    };
    let shots: Vec<Shot> = (0..14u32)
        .map(|i| {
            let stem = format!("IMG_{:04}", 4810 + i);
            let members = match i % 5 {
                0 | 3 => vec![
                    media(&format!("{stem}.CR3"), MediaKind::Raw, 28_400_000),
                    media(&format!("{stem}.JPG"), MediaKind::Photo, 7_100_000),
                ],
                4 => vec![media(
                    &format!("MVI_{:04}.MP4", 4810 + i),
                    MediaKind::Video,
                    184_000_000,
                )],
                2 if i > 10 => vec![media(&format!("{stem}.CR3"), MediaKind::Raw, 29_000_000)],
                _ => vec![media(&format!("{stem}.JPG"), MediaKind::Photo, 6_800_000)],
            };
            let shot = Shot {
                members,
                date: [2026, 9, 27 + i / 8, 9 + i, 12, 0],
                dated: true,
                camera: Some("Canon EOS R6".into()),
            };
            let c = colours[i as usize % colours.len()];
            g.averages.insert(
                shot.preview().into(),
                [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255],
            );
            shot
        })
        .collect();
    (g, shots)
}

/// Movie tiles: a play mark with the length in the bottom trailing corner ("0:09", "1:05",
/// "1:02:05", none when the header has none), beside photos; an HDR movie keeps its HDR
/// badge. Dark and light at 1× and 1.5×, and Hebrew (right to left) at 1×.
#[test]
fn video_tiles_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gallery-video",
    );
    let movies: [(&str, Option<u64>, bool); 6] = [
        ("IMG_0042.MOV", Some(65_400_000), false),
        ("clip.mp4", Some(9_000_000), false),
        ("concert.mp4", Some(3_725_000_000), true),
        ("phone.3gp", None, false),
        ("beach.m4v", Some(31_000_000), false),
        ("timelapse.mov", Some(600_000_000), false),
    ];
    let looks = [
        ("dark", Palette::dark(), false, 1.0, "1x"),
        ("dark", Palette::dark(), false, 1.5, "1.5x"),
        ("light", Palette::light(), false, 1.0, "1x"),
        ("light", Palette::light(), false, 1.5, "1.5x"),
        ("he-light", Palette::light(), true, 1.0, "1x"),
    ];
    for (look, pal, rtl, scale, suffix) in looks {
        let lang = if rtl {
            specular_i18n::langid!("he")
        } else {
            specular_i18n::langid!("en-US")
        };
        specular_i18n::I18n::with_thread_language(lang, || {
            let mut g = Gallery {
                active: true,
                ..Gallery::default()
            };
            let colours: [u32; 4] = [0x3d78b8, 0x6aa84f, 0xe69138, 0xc27ba0];
            let mut entries = Vec::new();
            for i in 0..9usize {
                let (name, movie) = match movies.get(i.wrapping_sub(i / 3 + 1)) {
                    Some(m) if i % 3 != 0 => (m.0.to_owned(), Some(*m)),
                    _ => (format!("photo-{i:02}.jpg"), None),
                };
                let path = PathBuf::from(format!("/shots/{name}"));
                let c = colours[i % colours.len()];
                g.averages.insert(
                    path.as_path().into(),
                    [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255],
                );
                if let Some((_, duration_us, hdr)) = movie {
                    g.metas.insert(
                        path.as_path().into(),
                        Arc::new(Meta {
                            format: "MPEG-4".into(),
                            width: 1920,
                            height: 1080,
                            video: true,
                            duration_us,
                            hdr,
                            ..Meta::default()
                        }),
                    );
                }
                entries.push(Entry {
                    path,
                    name,
                    kind: EntryKind::Image,
                    len: 20_000_000 + i as u64 * 1000,
                    modified: None,
                    hidden: false,
                    taken: None,
                    dims: Some(if movie.is_some() {
                        (1920, 1080)
                    } else {
                        (4000, 3000)
                    }),
                });
            }
            g.add_entries(entries);
            g.resort();
            g.rebuild();
            g.atlas = Some(Atlas::new(4));
            let f = Frame {
                pal: &pal,
                now: 1.0,
                reduce_motion: true,
                size: SIZE,
                scale,
                rtl,
                text_scale: 1.0,
                backdrop: Backdrop::None,
            };
            for _ in 0..20 {
                let mut warm = DrawList::new(scale);
                h.text.begin_frame(scale);
                g.draw(&mut warm, &mut h.text, &f);
                if !g.cap_more {
                    break;
                }
            }
            let shot = h.render(
                SIZE,
                scale,
                |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas),
                |dl, text| {
                    g.draw(dl, text, &f);
                },
            );
            goldens.check(&format!("gallery-video-{look}-{suffix}"), &shot);
            let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
            g.a11y(&mut tree, 1.0);
            super::super::a11y_audit::expect_clean(
                &format!("gallery-video-{look}-{suffix}"),
                &tree,
                f.rtl,
            );
        });
    }
    goldens.finish();
}

mod compare_shots;
mod import_shots;
mod library_shots;
mod local_shots;
mod map_shots;
mod people_shots;
mod picker_shots;
mod similar_shots;
