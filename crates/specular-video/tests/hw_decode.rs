//! The hardware video decoder (D3D12 on Windows, VA-API on Linux) against exact references
//! that need no ffmpeg-made reference set (that one is `decode.rs`): H.264 clips made by the
//! in-tree encoder against the software H.264 decoder, and the JCT-VC HEVC conformance
//! bitstreams (`tools/corpus/conformance/hevc/*.bit` as `tools/corpus/hevc-conformance.sh`
//! fetches them; any subset will do) against the in-tree HEVC decoder. Both software decoders
//! are exact, so every sample of every frame must match. Skips itself without a hardware
//! decoder; `SPECULAR_HEVC_CONF=<substring>` limits the conformance run.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use specular_core::CancelToken;
use specular_h264enc::{Config, Encoder, Frame, Profile};
use specular_heic::hevc;
use specular_hwdec::Error;
use specular_hwdec::video::{Chroma, VideoCodec, VideoDecoder, VideoFrame, video_supported};
use specular_video::mp4::{self, Colour};
use specular_video::mp4write::{Mp4Writer, Track};
use specular_video::sw_h264::SwH264;

/// Every sample of `samples` decoded on the hardware, each frame tagged with its index, as
/// they come out (decoding order); `Err(None)` when the hardware declines the stream,
/// `Err(Some(message))` when it fails on a sample.
fn hardware(
    codec: VideoCodec,
    config: &[u8],
    samples: &[Vec<u8>],
) -> Result<Vec<(i64, VideoFrame)>, Option<String>> {
    let mut dec = match VideoDecoder::new(codec, config) {
        Ok(d) => d,
        Err(Error::Unsupported(m)) => {
            eprintln!("  not in hardware: {m}");
            return Err(None);
        }
        Err(e) => return Err(Some(format!("open: {e}"))),
    };
    let mut got = Vec::new();
    for (i, s) in samples.iter().enumerate() {
        match dec.decode(s, i as i64) {
            Ok(()) => {}
            Err(Error::Unsupported(m)) => {
                eprintln!("  not in hardware from sample {i}: {m}");
                return Err(None);
            }
            Err(e) => return Err(Some(format!("sample {i}: {e}"))),
        }
        while let Some(f) = dec.next_output() {
            got.push(f);
        }
    }
    dec.flush();
    while let Some(f) = dec.next_output() {
        got.push(f);
    }
    Ok(got)
}

fn same(a: &VideoFrame, b: &VideoFrame) -> bool {
    (a.width, a.height, a.bit_depth, a.chroma) == (b.width, b.height, b.bit_depth, b.chroma)
        && a.planes == b.planes
}

/// A planar 4:2:0 picture: a smooth field with detail, panned and slowly zoomed, and a hard
/// cut every 20 frames.
fn picture(w: usize, h: usize, t: usize) -> [Vec<u8>; 3] {
    let scene = t / 20;
    let t = t % 20;
    let zoom = 1.0 + t as f32 * 0.004;
    let mut y = vec![0u8; w * h];
    for r in 0..h {
        for c in 0..w {
            let x = (c as f32 + t as f32 * 3.0) / zoom;
            let yy = r as f32 / zoom + scene as f32 * 41.0;
            let smooth = 110.0 + 60.0 * ((x * 0.021).sin() * (yy * 0.013).cos());
            let detail = 22.0 * (((x * 0.37).sin() + (yy * 0.29).cos()) * 0.5);
            let grain = ((x as usize * 7 + yy as usize * 13) % 11) as f32;
            y[r * w + c] = (smooth + detail + grain).clamp(16.0, 235.0) as u8;
        }
    }
    let (cw, ch) = (w / 2, h / 2);
    let mut u = vec![0u8; cw * ch];
    let mut v = vec![0u8; cw * ch];
    for r in 0..ch {
        for c in 0..cw {
            let x = (c as f32 + t as f32) / zoom;
            u[r * cw + c] = (128.0 + 40.0 * (x * 0.023 + scene as f32).sin()) as u8;
            v[r * cw + c] = (128.0 + 35.0 * (r as f32 * 0.031 - scene as f32).cos()) as u8;
        }
    }
    [y, u, v]
}

/// A clip of `n` frames encoded with `profile` at `w`×`h` (even; cropped when not a multiple
/// of 16), an IDR every `keyint` frames.
fn clip(profile: Profile, w: u32, h: u32, n: usize, keyint: u32) -> Vec<u8> {
    let mut cfg = Config::new(w, h, 30, 1, 24);
    cfg.profile = profile;
    cfg.keyint = keyint;
    let mut enc = Encoder::new(cfg).unwrap();
    let track = Track {
        width: w,
        height: h,
        fps_num: 30,
        fps_den: 1,
        avcc: enc.avcc(),
        colour: Colour {
            primaries: 1,
            transfer: 1,
            matrix: 1,
            full_range: false,
        },
    };
    let mut writer = Mp4Writer::new(Cursor::new(Vec::new()), track, n as u32).unwrap();
    for t in 0..n {
        let p = picture(w as usize, h as usize, t);
        let f = Frame {
            y: &p[0],
            u: &p[1],
            v: &p[2],
            y_stride: w as usize,
            c_stride: w as usize / 2,
        };
        let pk = enc.encode(&f, false);
        writer.write_sample(&pk.data, pk.key).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

/// Clips of each profile, with references across several frames and IDRs mid-stream, sample
/// for sample against the software decoder.
#[test]
fn h264_matches_software() {
    specular_hwdec::set_enabled(true);
    if !video_supported(VideoCodec::H264) {
        eprintln!("skipped: no hardware H.264 decoder");
        return;
    }
    let cancel = CancelToken::new();
    for (profile, w, h) in [
        (Profile::Baseline, 320, 240),
        (Profile::Main, 328, 200),
        (Profile::High, 200, 136),
    ] {
        let movie = clip(profile, w, h, 36, 14);
        let m = mp4::parse(&movie, true).unwrap();
        let v = m.video.as_ref().unwrap();
        let samples: Vec<Vec<u8>> = v
            .samples
            .iter()
            .map(|s| mp4::sample_bytes(&movie, s).unwrap().to_vec())
            .collect();
        let mut sw = SwH264::new(&v.config).unwrap();
        let mut want = Vec::new();
        for (i, s) in samples.iter().enumerate() {
            sw.step(Some((s, i as i64)), &cancel, &mut want).unwrap();
        }
        sw.step(None, &cancel, &mut want).unwrap();
        want.sort_by_key(|f| f.0);
        let mut got = match hardware(VideoCodec::H264, &v.config, &samples) {
            Ok(g) => g,
            Err(None) => return,
            Err(Some(e)) => panic!("{profile:?} {w}×{h}: {e}"),
        };
        got.sort_by_key(|f| f.0);
        assert_eq!(got.len(), want.len(), "{profile:?} {w}×{h}: frame count");
        for ((ta, a), (tb, b)) in got.iter().zip(&want) {
            assert_eq!(ta, tb, "{profile:?} {w}×{h}: sample order");
            assert!(
                same(a, b),
                "{profile:?} {w}×{h}: sample {ta} differs from software"
            );
        }
        eprintln!("{profile:?} {w}×{h}: {} frames match software", got.len());
    }
}

/// The NAL units of an Annex B byte stream, without start codes (trailing zero bytes, which
/// belong to the next start code, are dropped).
fn split_annexb(data: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 2 < data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut out = Vec::with_capacity(starts.len());
    for (k, &s) in starts.iter().enumerate() {
        let mut e = starts.get(k + 1).map_or(data.len(), |&n| n - 3);
        while e > s && data[e - 1] == 0 {
            e -= 1;
        }
        if e > s {
            out.push(&data[s..e]);
        }
    }
    out
}

/// Groups NAL units into access units (7.4.2.4.4): a new one starts at the first slice
/// segment of a picture, or at a parameter set, access unit delimiter, prefix SEI or
/// reserved prefix type that follows a picture's slices.
fn access_units<'a>(nals: &[&'a [u8]]) -> Vec<Vec<&'a [u8]>> {
    let mut aus: Vec<Vec<&[u8]>> = Vec::new();
    let mut cur: Vec<&[u8]> = Vec::new();
    let mut seen_vcl = false;
    for &n in nals {
        if n.len() < 2 {
            continue;
        }
        let t = (n[0] >> 1) & 0x3f;
        let layer = ((n[0] & 1) << 5) | (n[1] >> 3);
        let starts = if t < 32 {
            layer == 0 && n.len() > 2 && n[2] & 0x80 != 0
        } else {
            matches!(t, 32..=35 | 39 | 41..=44 | 48..=55)
        };
        if starts && seen_vcl {
            aus.push(std::mem::take(&mut cur));
            seen_vcl = false;
        }
        if t < 32 && layer == 0 {
            seen_vcl = true;
        }
        cur.push(n);
    }
    if !cur.is_empty() {
        aus.push(cur);
    }
    aus
}

fn nal_type(n: &[u8]) -> u8 {
    (n[0] >> 1) & 0x3f
}

/// An `hvcC` record carrying `sets` (VPS/SPS/PPS NAL units), 4-byte sample lengths; the
/// profile fields are nominal, the decoders read the SPS.
fn hvcc(sets: &[&[u8]]) -> Vec<u8> {
    let mut r = vec![
        1, 0x01, 0x60, 0, 0, 0, 0x90, 0, 0, 0, 0, 0, 93, 0xF0, 0, 0xFC, 0xFD, 0xF8, 0xF8, 0, 0,
        0x0F,
    ];
    let types: Vec<u8> = [32u8, 33, 34]
        .into_iter()
        .filter(|t| sets.iter().any(|n| nal_type(n) == *t))
        .collect();
    r.push(types.len() as u8);
    for t in types {
        let of: Vec<&&[u8]> = sets.iter().filter(|n| nal_type(n) == t).collect();
        r.push(0x80 | t);
        r.extend((of.len() as u16).to_be_bytes());
        for n in of {
            r.extend((n.len() as u16).to_be_bytes());
            r.extend_from_slice(n);
        }
    }
    r
}

/// An access unit as a sample: each NAL unit behind its 4-byte length.
fn sample(au: &[&[u8]]) -> Vec<u8> {
    let mut s = Vec::new();
    for n in au {
        s.extend((n.len() as u32).to_be_bytes());
        s.extend_from_slice(n);
    }
    s
}

/// A software-decoded picture in the hardware frames' layout: cropped planes, 8-bit samples
/// as bytes, deeper ones as little-endian words.
fn soft_frame(f: &hevc::Frame) -> VideoFrame {
    let plane = |p: &[u16], stride: u32, w: u32, rows: u32, wide: bool| {
        let mut out = Vec::with_capacity((w * rows) as usize * if wide { 2 } else { 1 });
        for r in 0..rows as usize {
            let row = &p[r * stride as usize..][..w as usize];
            if wide {
                out.extend(row.iter().flat_map(|s| s.to_le_bytes()));
            } else {
                out.extend(row.iter().map(|&s| s as u8));
            }
        }
        out
    };
    let wide = f.bit_depth > 8;
    let (cw, ch) = f.chroma.chroma_size(f.width, f.height);
    VideoFrame {
        width: f.width,
        height: f.height,
        bit_depth: f.bit_depth,
        chroma: Chroma::Yuv420,
        planes: [
            plane(&f.y, f.y_stride, f.width, f.height, wide),
            plane(&f.cb, f.c_stride, cw, ch, wide),
            plane(&f.cr, f.c_stride, cw, ch, wide),
        ],
        signal: None,
    }
}

/// The JCT-VC conformance streams on hand, as a motion clip would carry them, against the
/// in-tree decoder, access unit by access unit (the same pictures output, every one the
/// same). Streams the hardware declines (range extensions, sizes outside its limits) only
/// have to decode in software.
#[test]
fn hevc_conformance_matches_software() {
    specular_hwdec::set_enabled(true);
    if !video_supported(VideoCodec::Hevc) {
        eprintln!("skipped: no hardware HEVC decoder");
        return;
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/conformance/hevc");
    let only = std::env::var("SPECULAR_HEVC_CONF").ok();
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|d| {
            d.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|e| e == "bit"))
                .filter(|p| {
                    only.as_ref()
                        .is_none_or(|o| p.to_string_lossy().contains(o.as_str()))
                })
                .collect()
        })
        .unwrap_or_default();
    paths.sort();
    if paths.is_empty() {
        eprintln!("skipped: no streams in {}", dir.display());
        return;
    }
    let mut bad = Vec::new();
    let (mut compared, mut sw_only) = (0, 0);
    for path in &paths {
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let data = std::fs::read(path).unwrap();
        let aus = access_units(&split_annexb(&data));
        let sets: Vec<&[u8]> = aus
            .first()
            .map(|au| {
                au.iter()
                    .copied()
                    .filter(|n| matches!(nal_type(n), 32..=34))
                    .collect()
            })
            .unwrap_or_default();
        let mut sw = match hevc::VideoDecoder::new(&sets, 4) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{name}: not decoded in software ({e})");
                continue;
            }
        };
        let mut want = Vec::new();
        let mut unsupported = false;
        for (i, au) in aus.iter().enumerate() {
            match sw.decode_tagged(au, i as i64, None) {
                Ok(()) => {}
                Err(hevc::Error::Unsupported(m)) => {
                    eprintln!("{name}: not decoded in software ({m})");
                    unsupported = true;
                    break;
                }
                Err(e) => eprintln!("{name}: access unit {i}: {e}"),
            }
            while let Some(p) = sw.next_output() {
                want.push((p.tag, soft_frame(&p.frame)));
            }
        }
        if unsupported {
            continue;
        }
        sw.flush();
        while let Some(p) = sw.next_output() {
            want.push((p.tag, soft_frame(&p.frame)));
        }
        let samples: Vec<Vec<u8>> = aus.iter().map(|au| sample(au)).collect();
        let t0 = std::time::Instant::now();
        let mut got = match hardware(VideoCodec::Hevc, &hvcc(&sets), &samples) {
            Ok(g) => g,
            Err(None) => {
                sw_only += 1;
                continue;
            }
            Err(Some(e)) => {
                bad.push(format!("{name}: {e}"));
                continue;
            }
        };
        let took = t0.elapsed();
        compared += 1;
        // The hardware hands pictures out in decoding order (the player places them by
        // composition time), the software decoder in output order: compare by access unit.
        got.sort_by_key(|f| f.0);
        want.sort_by_key(|f| f.0);
        let tags = |v: &[(i64, VideoFrame)]| v.iter().map(|f| f.0).collect::<Vec<_>>();
        let (g, w) = (tags(&got), tags(&want));
        if g != w {
            let hw_extra: Vec<i64> = g.iter().copied().filter(|t| !w.contains(t)).collect();
            let sw_extra: Vec<i64> = w.iter().copied().filter(|t| !g.contains(t)).collect();
            bad.push(format!(
                "{name}: access units output by the hardware only {hw_extra:?}, by the software only {sw_extra:?}"
            ));
            continue;
        }
        let mut wrong = 0;
        for ((ta, a), (_, b)) in got.iter().zip(&want) {
            if !same(a, b) {
                if wrong == 0 {
                    eprintln!("{name}: first mismatch at access unit {ta}");
                }
                wrong += 1;
            }
        }
        eprintln!(
            "{name}: {} frames of {}×{} {}-bit in {took:?}, {wrong} wrong",
            got.len(),
            want.first().map_or(0, |f| f.1.width),
            want.first().map_or(0, |f| f.1.height),
            want.first().map_or(0, |f| f.1.bit_depth),
        );
        if wrong != 0 {
            bad.push(format!("{name}: {wrong} of {} frames", got.len()));
        }
    }
    eprintln!("{compared} streams compared, {sw_only} software only");
    assert!(bad.is_empty(), "mismatches: {bad:#?}");
}
