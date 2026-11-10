//! Differential tests: files written by the `exr` crate, decoded by both `exr` and specular-exr.
//! Sample values must match bit for bit (NaN payloads aside).

mod common;

use common::*;
use exr::math::RoundingMode;
use exr::prelude::*;
use specular_exr as sx;

const COMPRESSIONS: [Compression; 8] = [
    Compression::Uncompressed,
    Compression::RLE,
    Compression::ZIP1,
    Compression::ZIP16,
    Compression::PIZ,
    Compression::PXR24,
    Compression::B44,
    Compression::B44A,
];

fn opts(
    part: usize,
    level: usize,
    slots: [Option<String>; 4],
    parallel: bool,
) -> sx::DecodeOptions {
    sx::DecodeOptions {
        part,
        level: sx::LevelSelect::Index(level),
        channels: sx::ChannelSelect::Slots(slots),
        parallel,
        ..Default::default()
    }
}

/// Decode every part, level and channel (four at a time) and compare with the oracle.
fn check_all(bytes: &[u8], what: &str) {
    let oracle = oracle_read(bytes);
    let info = sx::read_info(bytes).unwrap_or_else(|e| panic!("{what}: read_info: {e}"));
    assert_eq!(
        info.parts.len(),
        oracle.layer_data.len(),
        "{what}: part count"
    );
    for (pi, layer) in oracle.layer_data.iter().enumerate() {
        let part = &info.parts[pi];
        assert_eq!(part.width(), layer.size.0 as u64, "{what}: width");
        assert_eq!(part.height(), layer.size.1 as u64, "{what}: height");
        let list = &layer.channel_data.list;
        assert_eq!(part.channels.len(), list.len(), "{what}: channel count");
        for (li, lvl) in part.levels.iter().enumerate() {
            for (gi, group) in list.chunks(4).enumerate() {
                let mut slots: [Option<String>; 4] = Default::default();
                for (s, c) in slots.iter_mut().zip(group) {
                    *s = Some(c.name.to_string());
                }
                // Exercise the sequential path once per file.
                let parallel = !(li == 0 && gi == 0);
                let img = sx::decode_with_info(bytes, &info, &opts(pi, li, slots, parallel))
                    .unwrap_or_else(|e| panic!("{what}: part {pi} level {li}: {e}"));
                assert_eq!(img.damaged_chunks, 0, "{what}: damaged chunks");
                assert_eq!(
                    (img.width, img.height),
                    (lvl.width, lvl.height),
                    "{what}: level size"
                );
                let px = f32_pixels(&img);
                for (s, c) in group.iter().enumerate() {
                    let exp = oracle_level(&c.sample_data, lvl.lx as usize, lvl.ly as usize);
                    let tag = format!(
                        "{what}: part {pi} level {li} ({},{}) channel {}",
                        lvl.lx, lvl.ly, c.name
                    );
                    assert_slot(px, s, &exp, &tag);
                }
                if group.len() < 4 {
                    assert!(
                        px.chunks_exact(4).all(|p| p[3] == 1.0),
                        "{what}: implicit alpha"
                    );
                }
            }
        }
    }
}

fn rgba_layer(
    w: usize,
    h: usize,
    types: [Ty; 4],
    enc: Encoding,
    specials: bool,
) -> Layer<AnyChannels<FlatSamples>> {
    let names = ["R", "G", "B", "A"];
    let list = (0..4)
        .map(|i| {
            channel(
                names[i],
                types[i],
                w,
                h,
                11 + i as u64 * 7,
                specials && types[i] != Ty::U,
            )
        })
        .collect();
    Layer::new((w, h), LayerAttributes::named(""), enc, chans(list))
}

#[test]
fn every_compression_and_pixel_type_scanline() {
    for c in COMPRESSIONS {
        for ty in [Ty::H, Ty::F, Ty::U] {
            for (w, h) in [(157, 93), (1, 1), (33, 1)] {
                let enc = encoding(c, None, LineOrder::Increasing);
                let bytes = write_one(rgba_layer(w, h, [ty; 4], enc, true));
                check_all(&bytes, &format!("scanline {c:?} {ty:?} {w}x{h}"));
            }
        }
    }
}

#[test]
fn every_compression_and_pixel_type_tiled() {
    for c in COMPRESSIONS {
        for ty in [Ty::H, Ty::F, Ty::U] {
            let enc = encoding(c, Some((64, 32)), LineOrder::Increasing);
            let bytes = write_one(rgba_layer(150, 77, [ty; 4], enc, true));
            check_all(&bytes, &format!("tiled {c:?} {ty:?}"));
        }
    }
}

#[test]
fn mixed_pixel_types() {
    for c in COMPRESSIONS {
        for tiles in [None, Some((16, 16))] {
            let enc = encoding(c, tiles, LineOrder::Increasing);
            let bytes = write_one(rgba_layer(70, 45, [Ty::H, Ty::F, Ty::U, Ty::H], enc, true));
            check_all(&bytes, &format!("mixed {c:?} {tiles:?}"));
        }
    }
}

#[test]
fn smooth_and_flat_images() {
    // Flat regions exercise B44A's 3-byte blocks, RLE runs and PIZ's run-length code.
    for c in COMPRESSIONS {
        let (w, h) = (128, 64);
        let flat: Vec<f32> = (0..w * h)
            .map(|i| {
                if i % w < 64 {
                    0.5
                } else {
                    (i / w) as f32 * 0.01
                }
            })
            .collect();
        let list = vec![
            AnyChannel::new("Y", samples(Ty::H, &flat)),
            AnyChannel::new("Z", samples(Ty::F, &vec![1.0; w * h])),
        ];
        let enc = encoding(c, None, LineOrder::Increasing);
        let bytes = write_one(Layer::new(
            (w, h),
            LayerAttributes::named(""),
            enc,
            chans(list),
        ));
        check_all(&bytes, &format!("flat {c:?}"));
    }
}

#[test]
fn decreasing_line_order() {
    for c in [
        Compression::Uncompressed,
        Compression::ZIP16,
        Compression::PIZ,
        Compression::B44,
    ] {
        for tiles in [None, Some((32, 32))] {
            let enc = encoding(c, tiles, LineOrder::Decreasing);
            let bytes = write_one(rgba_layer(90, 70, [Ty::H; 4], enc, false));
            let info = sx::read_info(&bytes).unwrap();
            assert_eq!(info.parts[0].line_order, sx::LineOrder::DecreasingY);
            check_all(&bytes, &format!("decreasing {c:?} {tiles:?}"));
        }
    }
}

#[test]
fn b44_quantize_linearly() {
    for c in [Compression::B44, Compression::B44A] {
        let (w, h) = (61, 37);
        let mut ch = channel("R", Ty::H, w, h, 5, false);
        ch.quantize_linearly = true;
        let list = vec![ch, channel("G", Ty::H, w, h, 6, false)];
        let enc = encoding(c, None, LineOrder::Increasing);
        let bytes = write_one(Layer::new(
            (w, h),
            LayerAttributes::named(""),
            enc,
            chans(list),
        ));
        assert!(sx::read_info(&bytes).unwrap().parts[0].channels[1].p_linear);
        check_all(&bytes, &format!("plinear {c:?}"));
    }
}

fn mip_layer(
    w: usize,
    h: usize,
    rip: bool,
    round: RoundingMode,
    c: Compression,
) -> Layer<AnyChannels<Levels<FlatSamples>>> {
    let list = ["B", "G", "R"]
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let levels = if rip {
                let sizes: Vec<_> = exr::meta::rip_map_levels(round, Vec2(w, h)).collect();
                let count = Vec2(
                    sizes.iter().map(|(i, _)| i.0).max().unwrap() + 1,
                    sizes.iter().map(|(i, _)| i.1).max().unwrap() + 1,
                );
                let map_data = sizes
                    .iter()
                    .map(|(idx, s)| {
                        samples(
                            Ty::H,
                            &values(s.0, s.1, (i * 100 + idx.0 * 10 + idx.1) as u64, false),
                        )
                    })
                    .collect();
                Levels::Rip {
                    level_data: RipMaps {
                        map_data,
                        level_count: count,
                    },
                    rounding_mode: round,
                }
            } else {
                let level_data = exr::meta::mip_map_levels(round, Vec2(w, h))
                    .map(|(idx, s)| samples(Ty::F, &values(s.0, s.1, (i * 100 + idx) as u64, true)))
                    .collect();
                Levels::Mip {
                    level_data,
                    rounding_mode: round,
                }
            };
            AnyChannel::new(*name, levels)
        })
        .collect::<Vec<_>>();
    let enc = encoding(c, Some((16, 8)), LineOrder::Increasing);
    Layer::new(
        (w, h),
        LayerAttributes::named(""),
        enc,
        AnyChannels::sort(SmallVec::from_vec(list)),
    )
}

fn write_levels(layer: Layer<AnyChannels<Levels<FlatSamples>>>) -> Vec<u8> {
    write_exr!(Image::from_layer(layer))
}

#[test]
fn mipmaps() {
    for round in [RoundingMode::Down, RoundingMode::Up] {
        for c in [Compression::ZIP16, Compression::PIZ, Compression::RLE] {
            let bytes = write_levels(mip_layer(75, 41, false, round, c));
            let info = sx::read_info(&bytes).unwrap();
            assert_eq!(
                info.parts[0].levels.len(),
                if round == RoundingMode::Down { 7 } else { 8 }
            );
            check_all(&bytes, &format!("mip {round:?} {c:?}"));
            // Preview: the smallest level at least 20x10.
            let o = sx::DecodeOptions {
                level: sx::LevelSelect::MinSize {
                    width: 20,
                    height: 10,
                },
                ..Default::default()
            };
            let img = sx::decode(&bytes, &o).unwrap();
            assert!(
                img.width >= 20 && img.height >= 10 && img.width < 40,
                "{}x{}",
                img.width,
                img.height
            );
        }
    }
}

#[test]
fn ripmaps() {
    for round in [RoundingMode::Down, RoundingMode::Up] {
        for c in [
            Compression::Uncompressed,
            Compression::ZIP1,
            Compression::B44A,
        ] {
            let bytes = write_levels(mip_layer(50, 23, true, round, c));
            check_all(&bytes, &format!("rip {round:?} {c:?}"));
        }
    }
}

#[test]
fn multipart_layers_and_windows() {
    let (w, h) = (40, 30);
    let mut a = rgba_layer(
        w,
        h,
        [Ty::H; 4],
        encoding(Compression::PIZ, None, LineOrder::Increasing),
        false,
    );
    a.attributes.layer_name = Some("beauty".into());
    a.attributes.layer_position = Vec2(-7, 3);
    let list = vec![
        channel("diffuse.R", Ty::F, 25, 20, 1, false),
        channel("diffuse.G", Ty::F, 25, 20, 2, false),
        channel("diffuse.B", Ty::F, 25, 20, 3, false),
        channel("depth.Z", Ty::F, 25, 20, 4, false),
    ];
    let mut b = Layer::new(
        (25, 20),
        LayerAttributes::named("aovs"),
        encoding(Compression::ZIP16, Some((8, 8)), LineOrder::Increasing),
        chans(list),
    );
    b.attributes.layer_position = Vec2(100, -50);
    let mut attrs = ImageAttributes::new(IntegerBounds::new((-10, -10), (64, 48)));
    attrs.chromaticities = Some(exr::meta::attribute::Chromaticities {
        red: Vec2(0.708, 0.292),
        green: Vec2(0.170, 0.797),
        blue: Vec2(0.131, 0.046),
        white: Vec2(0.3127, 0.3290),
    });
    let bytes = write_exr!(Image::empty(attrs).with_layer(a).with_layer(b));
    check_all(&bytes, "multipart");

    let info = sx::read_info(&bytes).unwrap();
    assert!(info.multipart);
    assert_eq!(info.parts[0].name.as_deref(), Some("beauty"));
    assert_eq!(info.parts[1].name.as_deref(), Some("aovs"));
    let dw = info.parts[0].data_window;
    assert_eq!(
        (dw.x_min, dw.y_min, dw.x_max, dw.y_max),
        (-7, 3, -7 + 39, 3 + 29)
    );
    let disp = info.parts[0].display_window;
    assert_eq!(
        (disp.x_min, disp.y_min, disp.x_max, disp.y_max),
        (-10, -10, 53, 37)
    );
    let ch = info.parts[0].chromaticities.expect("chromaticities");
    assert_eq!(ch.red, [0.708, 0.292]);
    assert_eq!(info.parts[1].layers(), vec!["depth", "diffuse"]);

    // Auto in part 1: no root colour, so the first layer with colour ("diffuse").
    let img = sx::decode(
        &bytes,
        &sx::DecodeOptions {
            part: 1,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(img.channels[0].as_deref(), Some("diffuse.R"));
    assert!(!img.has_alpha);
    let img = sx::decode(
        &bytes,
        &sx::DecodeOptions {
            part: 1,
            channels: sx::ChannelSelect::Layer("depth".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(img.color, sx::ColorKind::Luminance);
    assert_eq!(img.channels[1].as_deref(), Some("depth.Z"));
    assert!(
        sx::decode(
            &bytes,
            &sx::DecodeOptions {
                part: 2,
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[test]
fn luminance_auto() {
    let (w, h) = (20, 10);
    let list = vec![
        channel("Y", Ty::H, w, h, 9, false),
        channel("A", Ty::H, w, h, 10, false),
    ];
    let bytes = write_one(Layer::new(
        (w, h),
        LayerAttributes::named(""),
        encoding(Compression::ZIP16, None, LineOrder::Increasing),
        chans(list),
    ));
    let img = sx::decode(&bytes, &Default::default()).unwrap();
    assert_eq!(img.color, sx::ColorKind::Luminance);
    assert!(img.has_alpha);
    let y = values(w, h, 9, false);
    let a = values(w, h, 10, false);
    for (i, p) in f32_pixels(&img).chunks_exact(4).enumerate() {
        let yy = f16::from_f32(y[i]).to_f32();
        assert_eq!(
            [p[0], p[1], p[2], p[3]],
            [yy, yy, yy, f16::from_f32(a[i]).to_f32()]
        );
    }
}

#[test]
fn f16_output_matches_f32_output() {
    for c in [Compression::ZIP16, Compression::PIZ, Compression::PXR24] {
        let enc = encoding(c, None, LineOrder::Increasing);
        let bytes = write_one(rgba_layer(64, 40, [Ty::H, Ty::F, Ty::U, Ty::H], enc, true));
        let a = sx::decode(&bytes, &Default::default()).unwrap();
        let b = sx::decode(
            &bytes,
            &sx::DecodeOptions {
                format: sx::SampleFormat::F16,
                ..Default::default()
            },
        )
        .unwrap();
        let sx::Pixels::F16(hb) = &b.pixels else {
            panic!()
        };
        for (x, &hbits) in f32_pixels(&a).iter().zip(hb) {
            let want = f16::from_f32(*x);
            let got = f16::from_bits(hbits);
            assert!(
                want.to_bits() == got.to_bits() || (want.is_nan() && got.is_nan()),
                "{c:?}: {x} -> {got} vs {want}"
            );
        }
    }
}

#[test]
fn info_without_decode() {
    let enc = encoding(Compression::PIZ, Some((32, 32)), LineOrder::Increasing);
    let bytes = write_one(rgba_layer(100, 50, [Ty::H; 4], enc, false));
    assert!(sx::probe(&bytes));
    assert!(!sx::probe(b"\x76\x2f\x31"));
    let info = sx::read_info(&bytes).unwrap();
    let p = &info.parts[0];
    assert_eq!(p.compression, sx::Compression::Piz);
    assert!(matches!(p.storage, sx::Storage::Tiled(_)));
    assert_eq!(
        p.channels
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        ["A", "B", "G", "R"]
    );
    assert_eq!(p.chunk_count, 4 * 2);
}
