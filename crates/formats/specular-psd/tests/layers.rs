//! Layer pixels: channels at the layer's rectangle on the canvas, transparency, user masks,
//! opacities, every compression, 8/16/32-bit, PSB, colour modes, damage and cancellation.

mod common;

use std::sync::atomic::AtomicBool;

use common::*;
use specular_psd::*;

const SEQ: Options = Options { parallel: false };
const PAR: Options = Options { parallel: true };

/// Channel rows over `[t, l, b, r]` from `f(x, y)` in canvas coordinates, 8-bit.
fn rows8(bounds: [i32; 4], f: impl Fn(i32, i32) -> u8) -> Vec<u8> {
    let [t, l, b, r] = bounds;
    (t..b)
        .flat_map(|y| (l..r).map(move |x| (x, y)))
        .map(|(x, y)| f(x, y))
        .collect()
}

fn red(x: i32, y: i32) -> u8 {
    (x * 7 + y * 3) as u8
}
fn green(x: i32, y: i32) -> u8 {
    (x ^ y) as u8
}
fn blue(x: i32, _y: i32) -> u8 {
    200u8.wrapping_sub(x as u8)
}
fn alpha(x: i32, y: i32) -> u8 {
    ((x + y) * 9) as u8
}

/// An RGB document with a background composite and the given layers.
fn doc(w: u32, h: u32, depth: u16, layers: Vec<LayerSpec>) -> Spec {
    let bps = depth as usize / 8;
    let planes = (0..3).map(|c| pattern(w, h, c + 1, bps)).collect();
    let mut s = Spec::new(w, h, depth, 3, planes);
    s.layer_count = layers.len() as i16;
    s.layers = layers;
    s
}

fn rgba_layer(name: &str, bounds: [i32; 4], comp: u16) -> LayerSpec {
    LayerSpec {
        name: name.into(),
        bounds,
        opacity: 255,
        compression: comp,
        channels: vec![
            (-1, rows8(bounds, alpha)),
            (0, rows8(bounds, red)),
            (1, rows8(bounds, green)),
            (2, rows8(bounds, blue)),
        ],
        ..Default::default()
    }
}

fn u8_planes(p: &Planes) -> &[u8] {
    match &p.samples {
        Samples::U8(v) => v,
        _ => panic!("not 8-bit"),
    }
}

/// Checks an 8-bit RGB layer decode against `red`/`green`/`blue` and `want_alpha(x, y)`.
fn check_rgb8(p: &Planes, bounds: [i32; 4], want_alpha: impl Fn(i32, i32) -> u8) {
    let (w, h) = (p.width as i32, p.height as i32);
    let s = u8_planes(p);
    let plane = (w * h) as usize;
    assert_eq!(p.plane_count(), 4);
    assert!(p.alpha);
    let [t, l, b, r] = bounds;
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            let inside = y >= t && y < b && x >= l && x < r;
            let a = s[3 * plane + i];
            if !inside {
                assert_eq!(a, 0, "alpha outside the layer at {x},{y}");
                continue;
            }
            assert_eq!(
                [s[i], s[plane + i], s[2 * plane + i]],
                [red(x, y), green(x, y), blue(x, y)],
                "colour at {x},{y}"
            );
            assert_eq!(a, want_alpha(x, y), "alpha at {x},{y}");
        }
    }
}

fn decode(f: &[u8], psd: &Psd, index: usize) -> Planes {
    let a = psd.decode_layer_planes(f, index, &SEQ, None).unwrap();
    let b = psd.decode_layer_planes(f, index, &PAR, None).unwrap();
    assert_eq!(
        u8_or_bytes(&a),
        u8_or_bytes(&b),
        "parallel and sequential differ"
    );
    a
}

fn u8_or_bytes(p: &Planes) -> Vec<u8> {
    match &p.samples {
        Samples::U8(v) => v.clone(),
        Samples::U16(v) => v.iter().flat_map(|s| s.to_le_bytes()).collect(),
        Samples::F32(v) => v.iter().flat_map(|s| s.to_le_bytes()).collect(),
    }
}

#[test]
fn rgb8_every_compression_and_off_canvas_bounds() {
    for comp in 0..4 {
        // Inside, hanging off the top-left and off the bottom-right.
        let rects = [[2, 3, 9, 20], [-4, -6, 5, 7], [10, 15, 30, 41]];
        let layers = rects
            .iter()
            .enumerate()
            .map(|(i, &b)| rgba_layer(&format!("L{i}"), b, comp))
            .collect();
        let f = write(&doc(33, 17, 8, layers));
        let psd = parse(&f).unwrap();
        // Top-most first.
        assert_eq!(psd.pixel_layers(), vec![2, 1, 0]);
        for (i, &b) in rects.iter().enumerate() {
            let p = decode(&f, &psd, i);
            assert!(!p.truncated, "comp {comp} layer {i}");
            check_rgb8(&p, b, alpha);
        }
    }
}

#[test]
fn no_transparency_channel_is_opaque_and_opacity_applies() {
    let b = [1, 1, 6, 9];
    let mut l = rgba_layer("bg", b, 1);
    l.channels.remove(0);
    l.opacity = 128;
    l.fill_opacity = Some(255);
    let mut l2 = rgba_layer("faded", b, 1);
    l2.opacity = 51;
    l2.fill_opacity = Some(128);
    let f = write(&doc(10, 8, 8, vec![l, l2]));
    let psd = parse(&f).unwrap();
    assert_eq!(psd.layers()[1].fill_opacity, 128);
    check_rgb8(&decode(&f, &psd, 0), b, |_, _| 128);
    let mul = |a: u8, b: u8| ((a as u32 * b as u32 + 127) / 255) as u8;
    check_rgb8(&decode(&f, &psd, 1), b, |x, y| {
        mul(mul(alpha(x, y), 51), 128)
    });
}

fn mask_value(x: i32, y: i32) -> u8 {
    if (x + y) % 2 == 0 { 255 } else { 64 }
}

#[test]
fn user_masks() {
    let mul = |a: u8, b: u8| ((a as u32 * b as u32 + 127) / 255) as u8;
    let b = [0, 0, 12, 12];
    let mb = [3, 2, 8, 10];
    let in_mask = |x: i32, y: i32| y >= mb[0] && y < mb[2] && x >= mb[1] && x < mb[3];
    for comp in 0..4 {
        for (default, flags) in [(0u8, 0u8), (255, 0), (0, 2)] {
            let mut l = rgba_layer("masked", b, comp);
            l.channels.push((-2, rows8(mb, mask_value)));
            l.mask = Some(MaskSpec {
                bounds: mb,
                default,
                flags,
                real: None,
                ..Default::default()
            });
            let f = write(&doc(12, 12, 8, vec![l]));
            let psd = parse(&f).unwrap();
            let m = psd.layers()[0].mask.unwrap();
            assert_eq!(
                (m.channel, m.default, m.disabled),
                (-2, default, flags == 2)
            );
            check_rgb8(&decode(&f, &psd, 0), b, |x, y| {
                if flags == 2 {
                    alpha(x, y)
                } else if in_mask(x, y) {
                    mul(alpha(x, y), mask_value(x, y))
                } else {
                    mul(alpha(x, y), default)
                }
            });
        }
    }
}

#[test]
fn real_user_mask_beside_a_vector_mask() {
    let mul = |a: u8, b: u8| ((a as u32 * b as u32 + 127) / 255) as u8;
    let b = [0, 0, 6, 6];
    let real = [1, 1, 4, 5];
    let mut l = rgba_layer("vector", b, 2);
    // The vector mask's rendering (ignored) and the real user mask.
    l.channels.push((-2, rows8(b, |_, _| 0)));
    l.channels.push((-3, rows8(real, |_, _| 100)));
    l.mask = Some(MaskSpec {
        bounds: b,
        default: 255,
        flags: 8,
        real: Some((real, 0, 0)),
        ..Default::default()
    });
    let f = write(&doc(6, 6, 8, vec![l]));
    let psd = parse(&f).unwrap();
    assert_eq!(psd.layers()[0].mask.unwrap().channel, -3);
    let inside = |x: i32, y: i32| (1..4).contains(&y) && (1..5).contains(&x);
    check_rgb8(&decode(&f, &psd, 0), b, |x, y| {
        if inside(x, y) {
            mul(alpha(x, y), 100)
        } else {
            0
        }
    });
}

#[test]
fn groups_adjustments_and_empty_layers_are_not_pixel_layers() {
    let b = [0, 0, 4, 4];
    let mut hidden = rgba_layer("hidden", b, 0);
    hidden.hidden = true;
    let mut adj = rgba_layer("levels", b, 0);
    adj.keys = vec![*b"levl"];
    let mut text = rgba_layer("text", b, 0);
    text.keys = vec![*b"TySh"];
    let layers = vec![
        LayerSpec {
            name: "</Layer group>".into(),
            section: 3,
            opacity: 255,
            ..Default::default()
        },
        hidden,
        adj,
        LayerSpec {
            name: "empty".into(),
            opacity: 255,
            ..Default::default()
        },
        text,
        LayerSpec {
            name: "Group".into(),
            section: 1,
            opacity: 255,
            ..Default::default()
        },
    ];
    let f = write(&doc(4, 4, 8, layers));
    let psd = parse(&f).unwrap();
    assert_eq!(psd.layers()[2].kind, LayerKind::Adjustment);
    assert_eq!(psd.pixel_layers(), vec![4, 1]);
    assert!(!psd.layers()[1].visible);
    // Hidden layers still decode.
    check_rgb8(&decode(&f, &psd, 1), b, alpha);
    for i in [0, 2, 3, 5, 99] {
        assert!(
            psd.decode_layer_planes(&f, i, &SEQ, None).is_err(),
            "layer {i}"
        );
    }
}

#[test]
fn psb_rle_and_zip() {
    for comp in [1, 3] {
        let b = [-2, 5, 9, 70];
        let mut s = doc(64, 8, 8, vec![rgba_layer("wide", b, comp)]);
        s.psb = true;
        let f = write(&s);
        let psd = parse(&f).unwrap();
        assert_eq!(psd.info().version, Version::Psb);
        check_rgb8(&decode(&f, &psd, 0), b, alpha);
    }
}

#[test]
fn sixteen_and_thirty_two_bit() {
    let b = [1, 2, 7, 9];
    let n = ((b[2] - b[0]) * (b[3] - b[1])) as usize;
    let v16 = |c: usize, i: usize| (c * 20000 + i * 917) as u16;
    let v32 = |c: usize, i: usize| c as f32 * 0.25 + i as f32 / 100.0;
    for (depth, lr16) in [(16u16, true), (16, false), (32, false)] {
        for comp in 0..4 {
            let chan = |c: usize| -> Vec<u8> {
                (0..n)
                    .flat_map(|i| match depth {
                        16 => v16(c, i).to_be_bytes().to_vec(),
                        _ => v32(c, i).to_be_bytes().to_vec(),
                    })
                    .collect()
            };
            let l = LayerSpec {
                name: "deep".into(),
                bounds: b,
                opacity: 255,
                compression: comp,
                channels: vec![(0, chan(0)), (1, chan(1)), (2, chan(2)), (-1, chan(3))],
                ..Default::default()
            };
            let mut s = doc(10, 9, depth, vec![l]);
            s.lr16 = lr16;
            let f = write(&s);
            let psd = parse(&f).unwrap();
            assert_eq!(psd.pixel_layers(), vec![0], "depth {depth} lr16 {lr16}");
            let p = psd.decode_layer_planes(&f, 0, &PAR, None).unwrap();
            assert!(!p.truncated);
            let plane = 90;
            for y in 0..9 {
                for x in 0..10 {
                    let inside = y >= b[0] && y < b[2] && x >= b[1] && x < b[3];
                    let at = (y * 10 + x) as usize;
                    let i = ((y - b[0]) * (b[3] - b[1]) + (x - b[1])) as usize;
                    for c in 0..4 {
                        match &p.samples {
                            Samples::U16(s) => {
                                let want = if inside {
                                    v16(c, i)
                                } else if c == 3 {
                                    0
                                } else {
                                    s[c * plane + at]
                                };
                                assert_eq!(
                                    s[c * plane + at],
                                    want,
                                    "16-bit c{c} {x},{y} comp {comp}"
                                );
                            }
                            Samples::F32(s) => {
                                let want = if inside {
                                    v32(c, i)
                                } else if c == 3 {
                                    0.0
                                } else {
                                    s[c * plane + at]
                                };
                                assert_eq!(
                                    s[c * plane + at],
                                    want,
                                    "32-bit c{c} {x},{y} comp {comp}"
                                );
                            }
                            Samples::U8(_) => panic!("8-bit planes for a {depth}-bit file"),
                        }
                    }
                }
            }
            // Through the composite's conversions.
            assert_eq!(p.to_rgba8().format, PixelFormat::Rgba8);
            if depth == 16 {
                assert_eq!(p.to_wide(false).unwrap().format, PixelFormat::Rgba16);
            }
        }
    }
}

#[test]
fn grey_and_cmyk_layers() {
    let b = [0, 1, 3, 4];
    // Grey: one colour channel.
    let mut s = Spec::new(5, 3, 8, 1, vec![pattern(5, 3, 1, 1)]);
    s.layer_count = 1;
    s.layers = vec![LayerSpec {
        name: "grey".into(),
        bounds: b,
        opacity: 255,
        channels: vec![(0, rows8(b, red)), (-1, rows8(b, alpha))],
        compression: 1,
        ..Default::default()
    }];
    let f = write(&s);
    let psd = parse(&f).unwrap();
    let p = psd.decode_layer_planes(&f, 0, &SEQ, None).unwrap();
    assert_eq!((p.color_channels, p.plane_count()), (1, 2));
    let s8 = u8_planes(&p);
    assert_eq!(s8[1], red(1, 0));
    assert_eq!(s8[15 + 1], alpha(1, 0));
    assert_eq!(s8[15], 0);
    assert_eq!(p.to_rgba8().format, PixelFormat::Rgba8);

    // CMYK: four colour channels, then transparency.
    let planes = (0..4).map(|c| pattern(5, 3, c + 1, 1)).collect();
    let mut s = Spec::new(5, 3, 8, 4, planes);
    s.layer_count = 1;
    s.layers = vec![LayerSpec {
        name: "ink".into(),
        bounds: b,
        opacity: 255,
        channels: (0..4)
            .map(|c| (c, rows8(b, |x, _| (c as i32 * 50 + x) as u8)))
            .collect(),
        compression: 2,
        ..Default::default()
    }];
    let f = write(&s);
    let psd = parse(&f).unwrap();
    let p = psd.decode_layer_planes(&f, 0, &SEQ, None).unwrap();
    assert_eq!(
        (p.mode, p.color_channels, p.plane_count()),
        (ColorMode::Cmyk, 4, 5)
    );
    let s8 = u8_planes(&p);
    assert_eq!(
        [s8[2], s8[15 + 2], s8[30 + 2], s8[45 + 2], s8[60 + 2]],
        [2, 52, 102, 152, 255]
    );
    assert_eq!(s8[60], 0);
}

#[test]
fn damaged_channels_are_grey_and_flagged() {
    let b = [0, 0, 8, 8];
    for comp in 0..4 {
        let f = write(&doc(8, 8, 8, vec![rgba_layer("cut", b, comp)]));
        let psd = parse(&f).unwrap();
        let ch = &psd.layers()[0].channels;
        // Cut the file in the middle of the last channel's data.
        let last = ch.last().unwrap().data.clone();
        let cut = &f[..last.start + (last.len() / 2).max(3)];
        let psd = parse(cut).unwrap();
        if psd.pixel_layers().is_empty() {
            continue;
        }
        let p = psd.decode_layer_planes(cut, 0, &PAR, None).unwrap();
        assert!(p.truncated, "comp {comp}");
        // Garbage compression words.
        let mut bad = f.clone();
        bad[last.start] = 0x7f;
        let psd = parse(&bad).unwrap();
        assert!(matches!(
            psd.decode_layer_planes(&bad, 0, &SEQ, None),
            Err(Error::Unsupported(_))
        ));
    }
}

#[test]
fn cancel_stops_the_decode() {
    let f = write(&doc(64, 64, 8, vec![rgba_layer("big", [0, 0, 64, 64], 1)]));
    let psd = parse(&f).unwrap();
    let stop = AtomicBool::new(true);
    assert_eq!(
        psd.decode_layer_planes(&f, 0, &PAR, Some(&stop))
            .unwrap_err(),
        Error::Cancelled
    );
    let go = AtomicBool::new(false);
    assert!(psd.decode_layer_planes(&f, 0, &PAR, Some(&go)).is_ok());
}

#[test]
fn huge_bounds_are_refused_not_allocated() {
    let mut l = rgba_layer("huge", [0, 0, 2, 2], 0);
    l.bounds = [i32::MIN, i32::MIN, i32::MAX, i32::MAX];
    let f = write(&doc(4, 4, 8, vec![l]));
    let psd = parse(&f).unwrap();
    assert!(matches!(
        psd.decode_layer_planes(&f, 0, &SEQ, None),
        Err(Error::Malformed(_))
    ));
}

/// A file without a merged image ("Maximize compatibility" off) shows the
/// composite of its layers (it was refused): normal and multiply blending, opacity, a
/// clipped layer limited to its base, a hidden layer, and an isolated group at half opacity,
/// against the W3C compositing formulas worked by hand.
#[test]
fn composite_from_layers_without_merged_image() {
    let (w, h) = (4u32, 1u32);
    let solid = |name: &str, x0: i32, x1: i32, rgb: [u8; 3], a: u8| LayerSpec {
        name: name.into(),
        bounds: [0, x0, 1, x1],
        opacity: 255,
        channels: vec![
            (-1, vec![a; (x1 - x0) as usize]),
            (0, vec![rgb[0]; (x1 - x0) as usize]),
            (1, vec![rgb[1]; (x1 - x0) as usize]),
            (2, vec![rgb[2]; (x1 - x0) as usize]),
        ],
        ..Default::default()
    };
    // Bottom-most first: a grey base over x 0..3, a multiplied half-red over 1..3, a blue
    // clipped to the multiply layer over 2..4 (shown at x 2 only), a hidden white layer, and
    // a group (normal, opacity 128) holding a green pixel at x 3.
    let base = solid("base", 0, 3, [200, 200, 200], 255);
    let mul = LayerSpec {
        blend: Some(*b"mul "),
        ..solid("mul", 1, 3, [255, 0, 0], 255)
    };
    let clip = LayerSpec {
        clipping: true,
        opacity: 128,
        ..solid("clip", 2, 4, [0, 0, 255], 255)
    };
    let hidden = LayerSpec {
        hidden: true,
        ..solid("hidden", 0, 4, [255, 255, 255], 255)
    };
    let divider = LayerSpec {
        name: "</Layer group>".into(),
        section: 3,
        ..Default::default()
    };
    let green = solid("green", 3, 4, [0, 255, 0], 255);
    let folder = LayerSpec {
        name: "group".into(),
        section: 1,
        opacity: 128,
        blend: Some(*b"norm"),
        ..Default::default()
    };
    let mut s = doc(
        w,
        h,
        8,
        vec![base, mul, clip, hidden, divider, green, folder],
    );
    s.resources
        .push((1057, version_info(false, "Adobe Photoshop")));
    let f = write(&s);
    let psd = parse(&f).unwrap();
    assert_eq!(psd.info().composite, Composite::NotMaximized);
    assert!(psd.composites_layers());
    assert!(psd.info().has_alpha);
    assert_eq!(psd.undrawn_layers(&f), 0);
    let img = psd.decode_rgba8(&f, &SEQ).unwrap();
    let px = |x: usize| img.data[x * 4..x * 4 + 4].to_vec();
    // x 0: the grey base alone.
    assert_eq!(px(0), [200, 200, 200, 255]);
    // x 1: red multiplied onto grey: (200, 0, 0).
    assert_eq!(px(1), [200, 0, 0, 255]);
    // x 2: the multiply layer with the clipped blue at half opacity atop it, then
    // multiplied onto grey: (127, 0, 128) × 200/255 = (100, 0, 100).
    let p2 = px(2);
    assert!(
        p2[0].abs_diff(100) <= 1 && p2[1] == 0 && p2[2].abs_diff(100) <= 1,
        "{p2:?}"
    );
    assert_eq!(p2[3], 255);
    // x 3: only the group's green, at the group's half opacity (the clipped blue stops at
    // its base's edge).
    let p3 = px(3);
    assert!(
        p3[0] == 0 && p3[1] == 255 && p3[2] == 0 && p3[3].abs_diff(128) <= 1,
        "{p3:?}"
    );
    // A visible fill or adjustment layer is counted as not drawn.
    let mut s2 = s.clone();
    s2.layers.insert(
        1,
        LayerSpec {
            name: "levels".into(),
            keys: vec![*b"levl"],
            ..Default::default()
        },
    );
    s2.layer_count += 1;
    let f2 = write(&s2);
    let psd2 = parse(&f2).unwrap();
    assert_eq!(psd2.undrawn_layers(&f2), 1);
    assert_eq!(psd2.decode_rgba8(&f2, &SEQ).unwrap().data, img.data);
}
