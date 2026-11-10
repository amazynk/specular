//! Against libavcodec: the synthetic streams of `tests/data` (see `gen_fixtures.sh`) decode
//! to its PCM within a few 16-bit steps. libavcodec dithers zero-bit mantissas with its own
//! noise, so the comparison runs without dither on our side; its noise then shows as a few
//! steps where the tones start and stop (the encoders leave many bins without bits there).
//! Between, from the third block to the sixth last, every stream is within 8 steps, the error white noise.

use std::path::Path;

use specular_ac3::{Decoder, Options};

fn decode(data: &[u8], piece: usize, opts: Options) -> (Vec<f32>, usize) {
    let mut d = Decoder::new(opts);
    let mut out = Vec::new();
    for c in data.chunks(piece) {
        d.decode(c, &mut out).unwrap();
    }
    d.flush(&mut out).unwrap();
    (out, d.channels())
}

/// (largest difference in 16-bit steps, signal-to-difference ratio in dB).
fn compare(got: &[f32], want: &[i16]) -> (i32, f64) {
    let mut worst = 0;
    let (mut sig, mut err) = (0f64, 0f64);
    for (g, &w) in got.iter().zip(want) {
        let g = (g * 32768.0).round().clamp(-32768.0, 32767.0) as i32;
        let d = g - i32::from(w);
        worst = worst.max(d.abs());
        sig += f64::from(w).powi(2);
        err += f64::from(d).powi(2);
    }
    (worst, 10.0 * (sig / err.max(1e-9)).log10())
}

fn s16(path: &Path) -> Vec<i16> {
    std::fs::read(path)
        .unwrap()
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect()
}

const PLAIN: Options = Options {
    stereo: false,
    levels: false,
    drc: 1.0,
    dither: false,
    lfe_mix: false,
};

#[test]
fn streams_match_libavcodec() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    for (name, ext, stereo_ref) in [
        ("s48", "ac3", false),
        ("cpl44", "ac3", false),
        ("m32", "ac3", false),
        ("c51", "ac3", true),
        ("clicks", "ac3", false),
        ("e51", "eac3", true),
        ("e44", "eac3", false),
    ] {
        let data = std::fs::read(dir.join(format!("{name}.{ext}"))).unwrap();
        let want = s16(&dir.join(format!("{name}.s16")));
        for piece in [data.len(), 1000, 77] {
            let (got, ch) = decode(&data, piece, PLAIN);
            assert_eq!(
                got.len(),
                want.len(),
                "{name}: sample count ({ch} channels)"
            );
            let (worst, snr) = compare(&got, &want);
            let onset = 512 * ch;
            let mid = onset..got.len() - 3 * onset;
            let (later, later_snr) = compare(&got[mid.clone()], &want[mid]);
            eprintln!(
                "{name} in pieces of {piece}: within {worst} steps, {snr:.1} dB; \
                 in between {later} steps, {later_snr:.1} dB"
            );
            assert!(
                worst <= 24 && snr > 60.0,
                "{name}: {worst} steps, {snr:.1} dB"
            );
            assert!(
                later <= 8 && later_snr > 64.0,
                "{name}: {later} steps in between"
            );
        }
        if stereo_ref {
            let want = s16(&dir.join(format!("{name}.st.s16")));
            let (got, ch) = decode(
                &data,
                data.len(),
                Options {
                    stereo: true,
                    ..PLAIN
                },
            );
            assert_eq!(ch, 2);
            assert_eq!(got.len(), want.len(), "{name}: stereo sample count");
            let (worst, snr) = compare(&got, &want);
            let mid = 1024..got.len() - 3072;
            let (later, later_snr) = compare(&got[mid.clone()], &want[mid]);
            eprintln!(
                "{name} folded to stereo: within {worst} steps, {snr:.1} dB; \
                 in between {later} steps, {later_snr:.1} dB"
            );
            assert!(
                worst <= 24 && snr > 60.0,
                "{name} stereo: {worst} steps, {snr:.1} dB"
            );
            assert!(
                later <= 8 && later_snr > 64.0,
                "{name} stereo: {later} steps in between"
            );
        }
    }
}

#[test]
fn damage_does_not_panic() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let mut seed = 0x1234_5678u32;
    for name in ["c51.ac3", "e51.eac3", "clicks.ac3"] {
        let data = std::fs::read(dir.join(name)).unwrap();
        for round in 0..60 {
            let mut d = data.clone();
            for _ in 0..(1 + round % 7) {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let at = seed as usize % d.len();
                d[at] ^= (seed >> 24) as u8 | 1;
            }
            let cut = d.len() - (round * 37) % d.len().min(2000);
            let mut dec = Decoder::default();
            let mut out = Vec::new();
            let _ = dec.decode(&d[..cut], &mut out);
            let _ = dec.flush(&mut out);
            assert!(out.iter().all(|v| v.is_finite() && v.abs() <= 1.0));
        }
    }
}

fn f32s(path: &Path) -> Vec<f32> {
    std::fs::read(path)
        .unwrap()
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// (largest difference in 16-bit steps, RMS difference in dB below full scale).
fn below_full_scale(got: &[f32], want: &[f32]) -> (f64, f64) {
    let (mut worst, mut err) = (0f64, 0f64);
    for (&g, &w) in got.iter().zip(want) {
        let d = f64::from(g) - f64::from(w);
        worst = worst.max(d.abs() * 32768.0);
        err += d * d;
    }
    let rms = (err / got.len().max(1) as f64).sqrt();
    (worst, -20.0 * rms.max(1e-30).log10())
}

/// With the dither flags cleared ([`specular_ac3::without_dither`]) neither decoder adds
/// random noise, and the two agree sample for sample: within a 16-bit step everywhere
/// (onsets, coupling, rematrixing, block switching, the stereo fold), the difference far
/// below full scale. With dither on, libavcodec's noise and ours differ by the noise itself
/// (the 30 to 45 dB of the FATE streams in `corpus.rs`).
#[test]
fn undithered_streams_match_libavcodec_exactly() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    for (name, stereo) in [
        ("s48", false),
        ("cpl44", false),
        ("clicks", false),
        ("c51", true),
    ] {
        let data = std::fs::read(dir.join(format!("{name}.ac3"))).unwrap();
        let nd = specular_ac3::without_dither(&data);
        assert_eq!(nd.len(), data.len());
        assert_ne!(nd, data, "{name}: its frames dither");
        let file = if stereo {
            format!("{name}.nd.st.f32")
        } else {
            format!("{name}.nd.f32")
        };
        let want = f32s(&dir.join(file));
        let opts = Options { stereo, ..PLAIN };
        let (got, _) = decode(&nd, nd.len(), opts);
        assert_eq!(got.len(), want.len(), "{name}: sample count");
        // Nothing is left to dither, and every frame still passes its CRCs.
        let (dithered, _) = decode(
            &nd,
            nd.len(),
            Options {
                dither: true,
                ..opts
            },
        );
        assert_eq!(got, dithered, "{name}: a dither flag is left");
        let (worst, below) = below_full_scale(&got, &want);
        eprintln!("{name} undithered: within {worst:.3} steps, {below:.1} dB below full scale");
        assert!(
            worst < 1.0 && below > 110.0,
            "{name}: {worst:.3} steps, {below:.1} dB"
        );
    }
}

/// The syncframes of a stream.
fn frames(data: &[u8]) -> Vec<&[u8]> {
    let mut v = Vec::new();
    let mut at = 0;
    while at + 8 <= data.len() {
        let n = specular_ac3::FrameInfo::parse(&data[at..])
            .expect("a frame")
            .frame_bytes;
        v.push(&data[at..at + n]);
        at += n;
    }
    v
}

/// Table E2.5: Ls, Rs, the Lrs/Rrs pair (bit 0 in the top bit).
const LS_RS_BACK: u16 = (1 << 12) | (1 << 11) | (1 << 9);

/// e51's frames, each followed by q48's as dependent substream 0 with `chanmap`.
fn spliced(chanmap: Option<u16>) -> Vec<u8> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let main = std::fs::read(dir.join("e51.eac3")).unwrap();
    let ext = std::fs::read(dir.join("q48.eac3")).unwrap();
    let (main, ext) = (frames(&main), frames(&ext));
    assert_eq!(main.len(), ext.len());
    let mut out = Vec::new();
    for (m, e) in main.iter().zip(&ext) {
        out.extend_from_slice(m);
        let dep = specular_ac3::as_dependent(e, 0, chanmap).expect("a dependent frame");
        let info = specular_ac3::FrameInfo::parse(&dep).unwrap();
        assert_eq!(
            (info.strmtyp, info.substreamid, info.chanmap),
            (1, 0, chanmap)
        );
        assert_eq!(info.frame_bytes, dep.len());
        out.extend_from_slice(&dep);
    }
    out
}

/// Channel `c` of `ch` interleaved channels.
fn channel(v: &[f32], ch: usize, c: usize) -> Vec<f32> {
    v.iter().skip(c).step_by(ch).copied().collect()
}

/// Dependent substreams (Annex E): e51 with q48's four channels as a dependent substream,
/// once mapped to Ls Rs Lrs Rrs (7.1) and once without a map (replacing L R Ls Rs). The
/// streams are made here from the two fixtures (no encoder at hand writes dependent
/// substreams); e71 is compared with libavcodec's decode of it, every channel; in pieces of
/// any size, and one frame with its dependent frame per call (as container samples come).
/// e5q has no oracle: libavcodec skips a dependent substream without a channel map (its
/// output is e51's alone), where A/52 maps it by its coding mode
/// (`dependent_channels_land_where_mapped` checks ours bit for bit).
#[test]
fn dependent_substreams_match_libavcodec() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    for (name, map, want_ch) in [("e71", Some(LS_RS_BACK), 8), ("e5q", None, 6)] {
        let data = spliced(map);
        let file = dir.join(format!("{name}.eac3"));
        if std::env::var_os("SPECULAR_BLESS").is_some() {
            std::fs::write(&file, &data).unwrap();
        }
        assert_eq!(data, std::fs::read(&file).unwrap(), "{name}: made again");
        let info = specular_ac3::probe(&data).unwrap();
        assert!(info.dependents);
        assert_eq!((info.channels, info.output_channels()), (want_ch, want_ch));
        let want: Option<Vec<i16>> = (name == "e71").then(|| {
            std::fs::read(dir.join(format!("{name}.s16")))
                .expect("run gen_fixtures.sh")
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]))
                .collect()
        });
        for piece in [data.len(), 1000, 77] {
            let Some(want) = &want else {
                break;
            };
            let (got, ch) = decode(&data, piece, PLAIN);
            assert_eq!(ch, want_ch, "{name}");
            assert_eq!(got.len(), want.len(), "{name}: sample count");
            let (worst, snr) = compare(&got, want);
            let onset = 512 * ch;
            let mid = onset..got.len() - 3 * onset;
            let (later, later_snr) = compare(&got[mid.clone()], &want[mid]);
            eprintln!(
                "{name} in pieces of {piece}: within {worst} steps, {snr:.1} dB; \
                 in between {later} steps, {later_snr:.1} dB"
            );
            assert!(
                worst <= 24 && snr > 60.0,
                "{name}: {worst} steps, {snr:.1} dB"
            );
            assert!(
                later <= 8 && later_snr > 64.0,
                "{name}: {later} steps in between"
            );
        }
        // One frame and its dependent frame per call: each call puts out its own frame.
        let mut d = Decoder::new(PLAIN);
        let mut got = Vec::new();
        for pair in frames(&data).chunks(2) {
            let before = got.len();
            for f in pair {
                d.decode(f, &mut got).unwrap();
            }
            assert_eq!(
                got.len() - before,
                1536 * want_ch,
                "{name}: one frame per call"
            );
        }
        assert!(d.extended());
        let (all, _) = decode(&data, data.len(), PLAIN);
        assert_eq!(got, all, "{name}: by frames = whole");
    }
}

/// The dependent substream's channels land where its map says, bit for bit: in e71 the
/// independent frame's L R C LFE stay, q48's L R become the sides, its Ls Rs the back
/// pair; in e5q (no map) q48's L R Ls Rs replace e51's, its C and LFE stay. The stereo
/// fold takes the back pair as surrounds.
#[test]
fn dependent_channels_land_where_mapped() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let (main, mch) = decode(
        &std::fs::read(dir.join("e51.eac3")).unwrap(),
        1 << 20,
        PLAIN,
    );
    let (ext, qch) = decode(
        &std::fs::read(dir.join("q48.eac3")).unwrap(),
        1 << 20,
        PLAIN,
    );
    assert_eq!((mch, qch), (6, 4));
    // WAVE order: e51 L R C LFE Ls Rs; q48 L R Ls Rs; 7.1 L R C LFE Lb Rb Ls Rs.
    let (e71, ch) = decode(&spliced(Some(LS_RS_BACK)), 1 << 20, PLAIN);
    assert_eq!(ch, 8);
    for (out, from, (n, c)) in [
        (0, &main, (6, 0)),
        (1, &main, (6, 1)),
        (2, &main, (6, 2)),
        (3, &main, (6, 3)),
        (4, &ext, (4, 2)),
        (5, &ext, (4, 3)),
        (6, &ext, (4, 0)),
        (7, &ext, (4, 1)),
    ] {
        assert_eq!(
            channel(&e71, 8, out),
            channel(from, n, c),
            "e71 channel {out}"
        );
    }
    let (e5q, ch) = decode(&spliced(None), 1 << 20, PLAIN);
    assert_eq!(ch, 6);
    for (out, from, (n, c)) in [
        (0, &ext, (4, 0)),
        (1, &ext, (4, 1)),
        (2, &main, (6, 2)),
        (3, &main, (6, 3)),
        (4, &ext, (4, 2)),
        (5, &ext, (4, 3)),
    ] {
        assert_eq!(
            channel(&e5q, 6, out),
            channel(from, n, c),
            "e5q channel {out}"
        );
    }
    // Lo/Ro from 7.1: without mixing metadata A/52's middle levels (−4.5 dB centre, −6 dB
    // surround), scaled so nothing overloads.
    let (st, ch) = decode(
        &spliced(Some(LS_RS_BACK)),
        1 << 20,
        Options {
            stereo: true,
            ..PLAIN
        },
    );
    assert_eq!(ch, 2);
    let (c, s) = (0.594_603_6f32, 0.5f32);
    let norm = 1.0 + c + 2.0 * s;
    let mut worst = 0f32;
    for (i, f) in e71.chunks_exact(8).enumerate() {
        let lo = (f[0] + c * f[2] + s * (f[6] + f[4])) / norm;
        let ro = (f[1] + c * f[2] + s * (f[7] + f[5])) / norm;
        worst = worst
            .max((st[2 * i] - lo).abs())
            .max((st[2 * i + 1] - ro).abs());
    }
    assert!(worst < 1e-6, "fold off by {worst}");
}
