//! Clipped highlights stay white when the stated white level sits above where the sensor
//! clips (Canon: one clip per ISO, below the 14-bit maximum on some bodies, and a few units
//! apart per channel). Without the clip handling a clipped green ends below full scale while
//! red and blue, multiplied by their white balance gains, reach it: magenta.

use specular_raw::{
    Cfa, DecodeOptions, DevelopStats, MatrixSource, OutputSize, RawImage, Rect, develop,
};

const W: u32 = 128;
const H: u32 = 128;
const BLACK: f32 = 512.0;
const IDENTITY: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn rect(x: u32, y: u32, w: u32, h: u32) -> Rect {
    Rect { x, y, w, h }
}

/// RGGB, white 16383: a dim ramp everywhere, and a clipped patch (x, y in 32..48) whose
/// samples sit at `clip` (R, G, B).
fn scene(clip: [u16; 3]) -> RawImage {
    let mut data = vec![0u16; (W * H) as usize];
    for y in 0..H {
        for x in 0..W {
            let pos = (y & 1) * 2 + (x & 1);
            let col = [0, 1, 1, 2][pos as usize];
            let v = if (32..48).contains(&x) && (32..48).contains(&y) {
                clip[col]
            } else {
                600 + ((x * 97 + y * 31) % 9000) as u16
            };
            data[(y * W + x) as usize] = v;
        }
    }
    RawImage {
        width: W,
        height: H,
        cpp: 1,
        data,
        window: rect(0, 0, W, H),
        cfa: Cfa::bayer("RGGB"),
        crop: rect(0, 0, W, H),
        black: [BLACK; 4],
        white: 16383.0,
        linearization: None,
        wb: Some([2.0, 1.0, 1.5]),
        rgb_cam: IDENTITY,
        matrix_source: MatrixSource::Identity,
        profile: None,
        orientation: 1,
        truncated: false,
        opcodes: None,
    }
}

fn opts(size: OutputSize) -> DecodeOptions<'static> {
    DecodeOptions {
        size,
        ..Default::default()
    }
}

/// The developed pixel at sensor position (x, y).
fn px(raw: &RawImage, size: OutputSize, x: u32, y: u32) -> [u16; 3] {
    let img = develop(raw, &opts(size)).unwrap();
    let (x, y) = match size {
        OutputSize::Full => (x, y),
        OutputSize::Half => (x / 2, y / 2),
    };
    let i = ((y * img.width + x) * 3) as usize;
    [img.data[i], img.data[i + 1], img.data[i + 2]]
}

#[test]
fn clipped_patch_below_the_white_level_is_white_not_magenta() {
    // EOS 6D at ISO 100: every channel clips at 15024; the table white is 15490.
    let raw = scene([15024; 3]);
    let stats = DevelopStats::of(&raw).unwrap();
    assert_eq!(stats.white(), 15024.0);
    for size in [OutputSize::Half, OutputSize::Full] {
        assert_eq!(px(&raw, size, 40, 40), [65535; 3], "{size:?}");
    }
    // Without the clip handling (what the stated white alone gives): green at 0.914.
    let g = ((15024.0 - BLACK) / (16383.0 - BLACK) * 65535.0).round() as u16;
    assert!(g < 60000);
}

/// Leica M8/M9 DNG: 8-bit codes through a LinearizationTable whose top (16256) sits below
/// the white level (16383). The pile-up is found on the codes and read through the table, so
/// clipped highlights are white (they were magenta; fidelity round 4 on raw.pixls.us).
#[test]
fn linearized_clip_below_the_white_level_is_white() {
    // Codes 0..=255 to 0..=16256, the scene's ramp kept in codes.
    let table: Vec<u16> = (0..256u32).map(|c| (c * 16256 / 255) as u16).collect();
    let mut raw = scene([255; 3]);
    for v in &mut raw.data {
        if *v > 255 {
            *v = (*v / 64).min(250);
        }
    }
    raw.black = [0.0; 4];
    raw.linearization = Some(table);
    let stats = DevelopStats::of(&raw).unwrap();
    assert_eq!(stats.white(), 16256.0);
    for size in [OutputSize::Half, OutputSize::Full] {
        assert_eq!(px(&raw, size, 40, 40), [65535; 3], "{size:?}");
    }
}

#[test]
fn channels_clipping_a_few_units_apart_are_all_full_scale() {
    // EOS 300D: red and one green clip at 4007, the others at 4002 (table white 4000). Here
    // red at 15030, green at 15024, blue at 15019, all in the top quarter.
    let raw = scene([15030, 15024, 15019]);
    let stats = DevelopStats::of(&raw).unwrap();
    assert_eq!(stats.white(), 15030.0);
    for size in [OutputSize::Half, OutputSize::Full] {
        assert_eq!(px(&raw, size, 40, 40), [65535; 3], "{size:?}");
    }
}

#[test]
fn unclipped_data_keeps_the_stated_white() {
    // No pile-up: the brightest samples are single values, so nothing counts as clipped and
    // the scale is the stated white.
    let mut raw = scene([600; 3]);
    raw.data[0] = 15500;
    raw.data[(W + 1) as usize] = 15400;
    let stats = DevelopStats::of(&raw).unwrap();
    assert_eq!(stats.white(), 16383.0);
    let img = develop(&raw, &opts(OutputSize::Full)).unwrap();
    // A green sample with neighbours far below: its own value, scaled by the stated white.
    let v = raw.data[1] as f32;
    let want = ((v - BLACK) / (16383.0 - BLACK) * 65535.0).round();
    assert!(
        (img.data[4] as f32 - want).abs() <= 1.0,
        "{} vs {want}",
        img.data[4]
    );
}

#[test]
fn a_pile_below_the_top_quarter_is_not_a_clip() {
    // A large flat patch at 6000 (a third of the range): exposure, not clipping.
    let raw = scene([6000; 3]);
    let stats = DevelopStats::of(&raw).unwrap();
    assert_eq!(stats.white(), 16383.0);
    let [r, g, b] = px(&raw, OutputSize::Half, 40, 40);
    let lin = (6000.0 - BLACK) / (16383.0 - BLACK) * 65535.0;
    let near = |v: u16, want: f32| (v as f32 - want).abs() <= 1.0;
    assert!(near(g, lin), "{g} vs {lin}");
    assert!(near(r, 2.0 * lin) && near(b, 1.5 * lin), "{r} {b} vs {lin}");
}

/// Through an identity LinearizationTable the clip is found as without one (it used to be
/// left alone, which kept the M8/M9 magenta).
#[test]
fn linearized_data_clips_like_stored() {
    let mut raw = scene([15024; 3]);
    raw.linearization = Some((0..16384u16).collect());
    let stats = DevelopStats::of(&raw).unwrap();
    assert_eq!(stats.white(), 15024.0);
}

#[test]
fn a_pile_above_the_stated_white_raises_it() {
    // The data proves the sensor reaches 16000 while the file says 15000: highlights between
    // the two are kept instead of clipped, and the pile is still white.
    let mut raw = scene([16000; 3]);
    raw.white = 15000.0;
    let stats = DevelopStats::of(&raw).unwrap();
    assert_eq!(stats.white(), 16000.0);
    assert_eq!(px(&raw, OutputSize::Half, 40, 40), [65535; 3]);
}

/// Olympus E-PL1: the clip is spread by noise over about 30 codes (3928–3958 there, about
/// 20 000 samples per channel), and only a handful sit at the top value. The band is a clip:
/// its samples are white and the white is the top value (fidelity round 6).
#[test]
fn a_clip_spread_by_noise_is_a_clip() {
    let mut raw = scene([0; 3]);
    for y in 32..48u32 {
        for x in 32..48u32 {
            // 15000..=15029, each value about 8 times; 15030 only at (32, 32).
            raw.data[(y * W + x) as usize] = 15000 + ((x * 7 + y * 13) % 30) as u16;
        }
    }
    raw.data[(32 * W + 32) as usize] = 15030;
    let stats = DevelopStats::of(&raw).unwrap();
    assert_eq!(stats.white(), 15030.0);
    for size in [OutputSize::Half, OutputSize::Full] {
        assert_eq!(px(&raw, size, 40, 40), [65535; 3], "{size:?}");
    }
}

/// DJI FC220: a few samples at one value (5, 7, 1 and 6 per channel at 16380) with nothing in
/// the codes below are a clip, however large the frame.
#[test]
fn a_small_isolated_pile_is_a_clip() {
    let mut raw = scene([600; 3]);
    for y in 40..44u32 {
        for x in 40..44u32 {
            raw.data[(y * W + x) as usize] = 16000;
        }
    }
    let stats = DevelopStats::of(&raw).unwrap();
    assert_eq!(stats.white(), 16000.0);
}

/// Bright data that runs evenly up to its largest value, a few samples sharing it: no clip,
/// the stated white stays (the old rule took 4 samples at the top for one).
#[test]
fn an_even_bright_tail_is_not_a_clip() {
    let mut raw = scene([600; 3]);
    for y in 0..56u32 {
        for x in 0..W {
            // 15000..=15499 evenly (about 4 samples per code in use and position).
            raw.data[(y * W + x) as usize] = 15000 + ((y * W + x) * 977 % 500) as u16;
        }
    }
    for i in [0usize, 2, 2 * W as usize, 2 * W as usize + 2] {
        raw.data[i] = 15499;
    }
    let stats = DevelopStats::of(&raw).unwrap();
    assert_eq!(stats.white(), 16383.0);
}

/// Sony ARW2 near the top: codes in steps of 32, a pile at 16340–16404 and a thin tail up to
/// 16596 (the NEX-5N file). The clip starts at the pile; the white is the tail's top.
#[test]
fn a_pile_with_a_tail_above_it_is_a_clip() {
    let mut raw = scene([600; 3]);
    raw.white = 16383.0;
    for y in 0..64u32 {
        for x in 0..W {
            let i = (y * W + x) as usize;
            raw.data[i] = match y {
                // The scene up to the pile, every 32nd code.
                0..16 => 15700 + 32 * ((x + y * 3) % 20) as u16,
                // The pile.
                16..48 => [16340, 16372, 16404][(x % 3) as usize],
                // The tail.
                48..52 => [16436, 16468, 16500, 16532][(x % 4) as usize],
                52 => 16596,
                _ => 600,
            };
        }
    }
    let stats = DevelopStats::of(&raw).unwrap();
    assert_eq!(stats.white(), 16596.0);
    for size in [OutputSize::Half, OutputSize::Full] {
        assert_eq!(px(&raw, size, 20, 30), [65535; 3], "{size:?}");
    }
}
