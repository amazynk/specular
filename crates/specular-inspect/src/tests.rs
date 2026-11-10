use specular_core::{PixelLayout, Raster, RasterPlane};

use crate::*;

#[test]
fn srgb_round_trips_every_code() {
    for c in 0..=255u8 {
        let lin = srgb_decode(c as f32 / 255.0);
        assert_eq!(to_byte(srgb_encode(lin)), c, "code {c}");
        assert_eq!(bin(srgb_encode(lin)), c as usize, "bin {c}");
    }
    assert_eq!(bin(-1.0), 0);
    assert_eq!(bin(2.0), 255);
}

#[test]
fn luma_weights_sum_to_one() {
    assert!((luma([1.0, 1.0, 1.0]) - 1.0).abs() < 1e-6);
    assert!(luma([0.0, 1.0, 0.0]) > luma([1.0, 0.0, 0.0]));
    assert!(luma([1.0, 0.0, 0.0]) > luma([0.0, 0.0, 1.0]));
}

fn lin(c: [u8; 3]) -> [f32; 3] {
    c.map(|v| srgb_decode(v as f32 / 255.0))
}

#[test]
fn binning_counts_channels_luma_and_clipping() {
    let mut h = Histogram::new();
    h.add_linear(lin([255, 0, 0]), 1, 254);
    h.add_linear(lin([128, 128, 128]), 1, 254);
    h.add_linear(lin([0, 0, 0]), 1, 254);
    h.add_linear(lin([10, 20, 30]), 1, 254);
    assert_eq!(h.total, 4);
    assert_eq!(h.bins(Channel::Red)[255], 1);
    assert_eq!(h.bins(Channel::Red)[128], 1);
    assert_eq!(h.bins(Channel::Green)[0], 2);
    assert_eq!(h.bins(Channel::Blue)[30], 1);
    // Grey has the same luma code as its channels.
    assert_eq!(h.bins(Channel::Luma)[128], 1);
    assert_eq!(h.bins(Channel::Luma)[0], 1);
    // Pure red: luma 0.2126 linear → code 127.
    assert_eq!(h.bins(Channel::Luma)[127], 1);
    // Red is clipped in the highlights (one channel at 255) and the shadows (two at 0).
    assert_eq!(h.clipped_high, 1);
    assert_eq!(h.clipped_low, 2);
    assert!((h.high_fraction() - 0.25).abs() < 1e-6);
    assert!((h.low_fraction() - 0.5).abs() < 1e-6);
    for c in Channel::ALL {
        assert_eq!(h.bins(c).iter().sum::<u32>(), 4, "{c:?}");
    }
}

#[test]
fn words_round_trip() {
    let mut h = Histogram::new();
    for i in 0..=255u8 {
        h.add_linear(lin([i, 255 - i, i / 2]), 3, 250);
    }
    let w = h.to_words();
    assert_eq!(w.len(), Histogram::WORDS);
    assert_eq!(Histogram::from_words(&w), Some(h));
    assert_eq!(Histogram::from_words(&w[..10]), None);
}

#[test]
fn stats_of_a_known_distribution() {
    let mut h = Histogram::new();
    for v in [10u8, 20, 20, 30, 200] {
        h.add_linear(lin([v, v, v]), 0, 255);
    }
    let s = h.stats(Channel::Red);
    assert_eq!(s.count, 5);
    assert_eq!(s.min, 10);
    assert_eq!(s.max, 200);
    assert_eq!(s.median, 20);
    assert!((s.mean - 56.0).abs() < 1e-4);
    let empty = Histogram::new().stats(Channel::Luma);
    assert_eq!(empty.count, 0);
    assert_eq!(Histogram::new().high_fraction(), 0.0);
}

#[test]
fn curves_ignore_end_spikes_and_cap_them() {
    let mut h = Histogram::new();
    // A big clipped spike at 255 and a modest hump at 100.
    for _ in 0..1000 {
        h.add_linear(lin([255, 255, 255]), 0, 255);
    }
    for _ in 0..50 {
        h.add_linear(lin([100, 100, 100]), 0, 255);
    }
    for _ in 0..25 {
        h.add_linear(lin([101, 101, 101]), 0, 255);
    }
    let peak = h.peak(&Channel::RGB, false);
    assert_eq!(peak, 50.0);
    let c = h.curve(Channel::Red, false, peak, 256);
    assert_eq!(c.len(), 256);
    assert_eq!(c[100], 1.0);
    assert_eq!(c[101], 0.5);
    assert_eq!(c[255], 1.0, "end spike capped at full height");
    assert_eq!(c[0], 0.0);
    // Fewer columns keep the tallest bin of each.
    let c2 = h.curve(Channel::Red, false, peak, 128);
    assert_eq!(c2.len(), 128);
    assert_eq!(c2[50], 1.0);
    // Log scale lifts small counts relative to big ones.
    let lp = h.peak(&Channel::RGB, true);
    let cl = h.curve(Channel::Red, true, lp, 256);
    assert!(cl[101] > 0.5 && cl[101] < 1.0);
}

#[test]
fn two_tone_images_normalise_to_the_end_bins() {
    let mut h = Histogram::new();
    for _ in 0..10 {
        h.add_linear(lin([0, 0, 0]), 0, 255);
        h.add_linear(lin([255, 255, 255]), 0, 255);
    }
    let peak = h.peak(&[Channel::Luma], false);
    assert_eq!(peak, 10.0);
    let c = h.curve(Channel::Luma, false, peak, 256);
    assert_eq!((c[0], c[255]), (1.0, 1.0));
    assert!(
        Histogram::new()
            .curve(Channel::Red, true, 0.0, 64)
            .iter()
            .all(|&v| v == 0.0)
    );
}

#[test]
fn peaking_threshold_falls_with_sensitivity() {
    assert!(peaking_threshold(1) > peaking_threshold(50));
    assert!(peaking_threshold(50) > peaking_threshold(100));
    assert!(peaking_threshold(100) > 0.0);
    assert_eq!(peaking_threshold(0), peaking_threshold(1));
}

#[test]
fn ycbcr_matches_the_renderer() {
    assert_eq!(ycbcr_to_rgb(128, 128, 128), [128, 128, 128]);
    assert_eq!(ycbcr_to_rgb(255, 128, 128), [255, 255, 255]);
    assert_eq!(ycbcr_to_rgb(76, 85, 255), [254, 0, 0]);
    assert_eq!(ycbcr_to_rgb(0, 0, 0), [0, 135, 0]);
}

fn plane(w: u32, h: u32, bpp: usize, div: u8, f: impl Fn(u32, u32) -> Vec<u8>) -> RasterPlane {
    let mut data = Vec::new();
    for y in 0..h {
        for x in 0..w {
            data.extend(f(x, y));
        }
    }
    RasterPlane {
        width: w,
        height: h,
        stride: w as usize * bpp,
        data,
        h_div: div,
        v_div: div,
    }
}

#[test]
fn source_pixels_of_every_layout() {
    let gray = Raster {
        width: 4,
        height: 2,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::Gray,
        planes: vec![plane(4, 2, 1, 1, |x, y| vec![(y * 4 + x) as u8])],
        truncated: false,
    };
    assert_eq!(source_pixel(&gray, 3, 1), Some(SourceValue::Gray(7)));
    assert_eq!(source_pixel(&gray, 4, 0), None);

    // A region at (10, 20) of a half-scale decode, with 2× subsampled chroma.
    let ycc = Raster {
        width: 4,
        height: 4,
        origin: (10, 20),
        scale: 2,
        layout: PixelLayout::YCbCr,
        planes: vec![
            plane(4, 4, 1, 1, |x, y| vec![(y * 4 + x) as u8 * 10]),
            plane(2, 2, 1, 2, |x, y| vec![100 + (y * 2 + x) as u8]),
            plane(2, 2, 1, 2, |_, _| vec![128]),
        ],
        truncated: false,
    };
    // Stored (25, 43) → scaled (12, 21) → raster (2, 1) → chroma (1, 0).
    let v = source_pixel(&ycc, 25, 43).unwrap();
    let SourceValue::YCbCr { ycc: s, rgb } = v else {
        panic!("{v:?}")
    };
    assert_eq!(s, [60, 101, 128]);
    assert_eq!(rgb, ycbcr_to_rgb(60, 101, 128));
    assert_eq!(source_pixel(&ycc, 19, 43), None, "left of the region");

    let rgba = Raster {
        width: 2,
        height: 1,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::Rgba,
        planes: vec![plane(2, 1, 4, 1, |x, _| vec![x as u8, 2, 3, 128])],
        truncated: false,
    };
    assert_eq!(
        source_pixel(&rgba, 1, 0),
        Some(SourceValue::Rgba([1, 2, 3, 128]))
    );
    let rgb = Raster {
        layout: PixelLayout::Rgb,
        planes: vec![
            plane(2, 1, 1, 1, |_, _| vec![9]),
            plane(2, 1, 1, 1, |_, _| vec![8]),
            plane(2, 1, 1, 1, |_, _| vec![7]),
        ],
        ..rgba.clone()
    };
    assert_eq!(source_pixel(&rgb, 0, 0), Some(SourceValue::Rgb([9, 8, 7])));
    // 16-bit layouts: native-endian samples, the whole 0..=65535 range.
    let ne = |v: &[u16]| v.iter().flat_map(|s| s.to_ne_bytes()).collect::<Vec<u8>>();
    let g16 = Raster {
        layout: PixelLayout::Gray16,
        planes: vec![plane(2, 1, 2, 1, |x, _| {
            ne(&[if x == 1 { 65535 } else { 300 }])
        })],
        ..rgba.clone()
    };
    assert_eq!(source_pixel(&g16, 1, 0), Some(SourceValue::Gray16(65535)));
    assert_eq!(source_pixel(&g16, 0, 0), Some(SourceValue::Gray16(300)));
    let ga16 = Raster {
        layout: PixelLayout::GrayAlpha16,
        planes: vec![plane(2, 1, 4, 1, |x, _| ne(&[x as u16 * 1000 + 7, 40000]))],
        ..rgba.clone()
    };
    assert_eq!(
        source_pixel(&ga16, 1, 0),
        Some(SourceValue::GrayAlpha16([1007, 40000]))
    );
    let rgba16 = Raster {
        layout: PixelLayout::Rgba16(specular_core::HdrEncoding::SRGB),
        planes: vec![plane(2, 1, 8, 1, |x, _| ne(&[x as u16, 2, 60000, 65535]))],
        ..rgba.clone()
    };
    let v = source_pixel(&rgba16, 1, 0).unwrap();
    assert_eq!(v, SourceValue::Rgba16([1, 2, 60000, 65535]));
    assert_eq!((v.bits(), SourceValue::Gray(1).bits()), (16, 8));
    let f16 = Raster {
        layout: PixelLayout::Rgba16F(specular_core::Primaries::Bt709),
        ..rgba16.clone()
    };
    assert_eq!(
        source_pixel(&f16, 0, 0),
        None,
        "half floats have no code value"
    );
    // YCbCr planes (HEIC): the R'G'B' and alpha the viewer converts them to, as codes.
    let e = specular_core::YuvEncoding {
        matrix: specular_core::YuvMatrix::Bt601,
        full_range: true,
        bits: 8,
        cosited_x: false,
        cosited_y: false,
        bottom_y: false,
        right_x: false,
        alpha: true,
        colour: specular_core::HdrEncoding::SRGB,
    };
    let yuv = Raster {
        layout: PixelLayout::Yuv(e),
        planes: vec![
            plane(2, 1, 1, 1, |x, _| vec![100 + x as u8]),
            plane(1, 1, 1, 2, |_, _| vec![128]),
            plane(1, 1, 1, 2, |_, _| vec![128]),
            plane(2, 1, 1, 1, |_, _| vec![77]),
        ],
        ..rgba.clone()
    };
    assert_eq!(
        source_pixel(&yuv, 1, 0),
        Some(SourceValue::Rgba([101, 101, 101, 77]))
    );
    // A malformed raster (missing planes) gives nothing rather than panicking.
    let broken = Raster {
        planes: vec![],
        ..rgb
    };
    assert_eq!(source_pixel(&broken, 0, 0), None);
}

#[test]
fn display_values_unpremultiply() {
    let half = srgb_decode(128.0 / 255.0);
    let d = DisplayValue::from_premultiplied([half * 0.5, 0.0, 0.5, 0.5], 0);
    assert_eq!(d.srgb, [128, 0, 255, 128]);
    assert!(d.exact);
    assert!(!DisplayValue::from_premultiplied([0.0; 4], 2).exact);
    assert_eq!(
        DisplayValue::from_premultiplied([0.0; 4], 0).srgb,
        [0, 0, 0, 0]
    );
    assert_eq!(hex([255, 8, 171]), "#FF08AB");
}

#[test]
fn channel_keys_toggle_and_alpha_cycles() {
    use ChannelKey as K;
    use ChannelView as V;
    for (key, view) in [
        (K::Red, V::Red),
        (K::Green, V::Green),
        (K::Blue, V::Blue),
        (K::Luma, V::Luma),
    ] {
        assert_eq!(V::Normal.press(key), view);
        assert_eq!(
            view.press(key),
            V::Normal,
            "{key:?} again turns {view:?} off"
        );
        // Another channel key switches straight to that channel.
        assert_eq!(V::Alpha.press(key), view);
        assert_eq!(V::AlphaMatte.press(key), view);
    }
    assert_eq!(V::Red.press(K::Green), V::Green);
    // Alpha: grey → checkerboard → matte → normal.
    let mut v = V::Normal;
    let mut seen = Vec::new();
    for _ in 0..4 {
        v = v.press(K::Alpha);
        seen.push(v);
    }
    assert_eq!(seen, [V::Alpha, V::AlphaChecker, V::AlphaMatte, V::Normal]);
    assert_eq!(V::Luma.press(K::Alpha), V::Alpha);
}

#[test]
fn alpha_views_fall_back_on_opaque_images() {
    for v in [
        ChannelView::Alpha,
        ChannelView::AlphaChecker,
        ChannelView::AlphaMatte,
    ] {
        assert!(v.is_alpha());
        assert_eq!(v.effective(false), ChannelView::Normal);
        assert_eq!(v.effective(true), v);
    }
    for v in [ChannelView::Normal, ChannelView::Red, ChannelView::Luma] {
        assert!(!v.is_alpha());
        assert_eq!(v.effective(false), v);
    }
}

#[test]
fn channel_shader_modes_and_grey_levels() {
    let modes: Vec<_> = [
        ChannelView::Normal,
        ChannelView::Red,
        ChannelView::Green,
        ChannelView::Blue,
        ChannelView::Alpha,
        ChannelView::AlphaChecker,
        ChannelView::AlphaMatte,
        ChannelView::Luma,
    ]
    .iter()
    .map(|v| v.shader_mode())
    .collect();
    assert_eq!(modes, [0, 1, 2, 3, 4, 0, 0, 5]);
    // Half-transparent orange: the colour views show its colour at full strength.
    let px = [0.8, 0.2, 0.0, 0.5];
    let near = |a: Option<f32>, b: f32| (a.unwrap() - b).abs() < 1e-6;
    assert!(near(ChannelView::Red.grey(px), 0.8));
    assert!(near(ChannelView::Green.grey(px), 0.2));
    assert!(near(ChannelView::Blue.grey(px), 0.0));
    assert!(near(ChannelView::Alpha.grey(px), 0.5));
    assert!(near(ChannelView::Luma.grey(px), luma([0.8, 0.2, 0.0])));
    assert_eq!(ChannelView::AlphaChecker.grey(px), None);
    // Fully transparent: the hidden colour, alpha 0.
    assert!(near(ChannelView::Blue.grey([0.0, 0.0, 0.7, 0.0]), 0.7));
    assert!(near(ChannelView::Alpha.grey([0.0, 0.0, 0.7, 0.0]), 0.0));
    // Out-of-gamut negatives (wide-gamut sources in the BT.709 working space) clamp to black.
    assert!(near(ChannelView::Red.grey([-0.2, 0.5, 0.5, 1.0]), 0.0));
}

#[test]
fn channel_colour_presets() {
    assert_eq!(ChannelColours::default(), ChannelColours::RedGreen);
    assert_eq!(ChannelColours::RedGreen.matrix(), None);
    assert_eq!(
        ChannelColours::RedGreen.apply([0.1, 0.2, 0.3]),
        [0.1, 0.2, 0.3]
    );
    // Magenta–green: the first channel lights red and blue, the second green.
    assert_eq!(
        ChannelColours::MagentaGreen.apply([1.0, 0.0, 0.0]),
        [1.0, 0.0, 1.0]
    );
    assert_eq!(
        ChannelColours::MagentaGreen.apply([0.0, 1.0, 0.0]),
        [0.0, 1.0, 0.0]
    );
    assert_eq!(
        ChannelColours::MagentaGreen.apply([0.0, 0.0, 1.0]),
        [0.0, 0.0, 0.0]
    );
    // Both at full make white; co-located signal stays neutral.
    assert_eq!(
        ChannelColours::MagentaGreen.apply([1.0, 1.0, 0.0]),
        [1.0, 1.0, 1.0]
    );
    // Magenta–yellow–cyan at half intensity: all three at full make white.
    let myc = ChannelColours::MagentaYellowCyan;
    assert_eq!(myc.apply([1.0, 0.0, 0.0]), [0.5, 0.0, 0.5]);
    assert_eq!(myc.apply([0.0, 1.0, 0.0]), [0.5, 0.5, 0.0]);
    assert_eq!(myc.apply([0.0, 0.0, 1.0]), [0.0, 0.5, 0.5]);
    assert_eq!(myc.apply([1.0, 1.0, 1.0]), [1.0, 1.0, 1.0]);
    for c in ChannelColours::ALL {
        assert_eq!(ChannelColours::from_id(c.id()), Some(c));
    }
    assert_eq!(myc.next(), ChannelColours::RedGreen);
}

/// The luma view weighs the linear BT.709 working space with BT.709 coefficients. Luminance
/// is a property of the colour, not of the primaries, so that equals the image's own
/// coefficients on its own primaries: e.g. BT.2020 weights on BT.2020 values.
#[test]
fn working_space_luma_is_the_sources_own_luminance() {
    use specular_core::hdr::{Primaries, to_bt709};
    let weights = [
        (Primaries::Bt2020, [0.2627, 0.6780, 0.0593]),
        (Primaries::DisplayP3, [0.228_975, 0.691_739, 0.079_287]),
    ];
    for (p, w) in weights {
        let m = to_bt709(p);
        for src in [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.3, 0.7, 0.2],
        ] {
            let own = w[0] * src[0] + w[1] * src[1] + w[2] * src[2];
            let lin709: [f32; 3] = std::array::from_fn(|r| (0..3).map(|c| m[r][c] * src[c]).sum());
            let y = luma(lin709);
            assert!((y - own).abs() < 2e-3, "{p:?} {src:?}: {y} vs {own}");
        }
    }
}
