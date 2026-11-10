//! Compressed Matroska tracks against the same tracks stored plainly: the fixtures in
//! tests/data are written again here, once plainly and once with header stripping, zlib (of
//! the frames and the codec private data) or both, each with every lacing, and the encoded
//! file must give the same samples, frames and sound, bit for bit.

use super::build::{el, uel};
use super::coding::tests::zlib;
use super::coding::{
    CONTENT_COMP_ALGO, CONTENT_COMP_SETTINGS, CONTENT_COMPRESSION, CONTENT_ENCODING,
    CONTENT_ENCODING_ORDER, CONTENT_ENCODING_SCOPE, CONTENT_ENCRYPTION,
};
use super::*;
use crate::mp4::Movie;
use specular_core::{CancelToken, Source};
use specular_decode::{Format, Frames, Session};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(name),
    )
    .unwrap()
}

/// How a track is stored in the written file.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Enc {
    Plain,
    /// The bytes every frame of the track starts with are stripped (at least one).
    Strip,
    /// Each frame and the codec private data zlib-compressed.
    Zlib,
    /// Stripped (order 0), then the rest zlib-compressed (order 1).
    StripZlib,
}

/// How a track's frames are laced (Matroska lacing bits).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Lace {
    None,
    Xiph,
    Fixed,
    Ebml,
}

fn vint(v: u64) -> Vec<u8> {
    let l = (1..=8).find(|&l| v < (1u64 << (7 * l)) - 1).unwrap();
    let x = v | (1u64 << (7 * l));
    x.to_be_bytes()[8 - l..].to_vec()
}

/// A signed EBML lace size difference.
fn svint(d: i64) -> Vec<u8> {
    let l = (1..=8)
        .find(|&l| {
            let bias = (1i64 << (7 * l - 1)) - 1;
            d + bias >= 0 && d + bias < (1i64 << (7 * l)) - 1
        })
        .unwrap();
    let raw = (d + (1i64 << (7 * l - 1)) - 1) as u64 | (1u64 << (7 * l));
    raw.to_be_bytes()[8 - l..].to_vec()
}

fn encoding(order: u64, scope: u64, algo: u64, settings: &[u8]) -> Vec<u8> {
    let mut comp = uel(CONTENT_COMP_ALGO, algo);
    if !settings.is_empty() {
        comp.extend(el(CONTENT_COMP_SETTINGS, settings));
    }
    let mut b = uel(CONTENT_ENCODING_ORDER, order);
    b.extend(uel(CONTENT_ENCODING_SCOPE, scope));
    b.extend(el(CONTENT_COMPRESSION, &comp));
    el(CONTENT_ENCODING, &b)
}

/// The longest prefix (up to 8 bytes) every frame starts with.
fn common_prefix(frames: &[&[u8]]) -> Vec<u8> {
    let mut p: Vec<u8> = frames
        .first()
        .map_or(&[][..], |f| &f[..f.len().min(8)])
        .to_vec();
    for f in frames {
        let n = p.iter().zip(f.iter()).take_while(|(a, b)| a == b).count();
        p.truncate(n);
    }
    p
}

/// `src` written again: its EBML header, `Info` and `Tracks` (each track stored as `enc`
/// says, with its `ContentEncodings`), and its frames in clusters of blocks, the frames of
/// the tracks in `laced` laced by `lace` in fours (fixed lacing only where they are the same
/// size as stored).
fn remux(src: &[u8], enc: Enc, lace: Lace, laced: &[u64]) -> Vec<u8> {
    let header = element(src, 0).unwrap();
    let mut out = src[..header.end].to_vec();
    let seg = element(src, header.end).unwrap();
    assert_eq!(seg.id, SEGMENT);
    let mut info = Vec::new();
    let mut tracks_el = None;
    let mut clusters = Vec::new();
    let mut p = seg.body;
    while let Some(e) = element(src, p).filter(|_| p < seg.end.min(src.len())) {
        match e.id {
            INFO => info = src[p..e.end].to_vec(),
            TRACKS => tracks_el = Some(e),
            CLUSTER => clusters.push(e),
            _ => {}
        }
        if e.unknown {
            p = cluster(src, e.body, e.end, true, &[], &mut Vec::new());
            continue;
        }
        p = e.end;
    }
    let tracks_el = tracks_el.unwrap();
    let numbers: Vec<u64> = children(src, tracks_el.body, tracks_el.end)
        .filter(|c| c.id == TRACK_ENTRY)
        .map(|c| track_entry(src, &c).number)
        .collect();
    let mut frames = Vec::new();
    for e in &clusters {
        cluster(src, e.body, e.end, e.unknown, &numbers, &mut frames);
    }
    let raw = |f: &Frame| &src[f.offset..f.offset + f.size];
    // What each track's frames start with.
    let prefix = |n: u64| {
        let fs: Vec<&[u8]> = frames
            .iter()
            .filter(|f| f.0 == n)
            .map(|f| raw(&f.1))
            .collect();
        common_prefix(&fs)
    };
    let mut strip: Vec<(u64, Vec<u8>)> = Vec::new();
    let mut entries = Vec::new();
    for c in children(src, tracks_el.body, tracks_el.end).filter(|c| c.id == TRACK_ENTRY) {
        let n = track_entry(src, &c).number;
        // Empty when the frames share no first byte: stripping then puts nothing back.
        let s = prefix(n);
        let mut body = Vec::new();
        for k in children(src, c.body, c.end) {
            match k.id {
                CONTENT_ENCODINGS => {}
                CODEC_PRIVATE if enc == Enc::Zlib => {
                    body.extend(el(CODEC_PRIVATE, &zlib(&src[k.body..k.end])));
                }
                _ => body.extend_from_slice(&src[start_of(src, &c, &k)..k.end]),
            }
        }
        let encs = match enc {
            Enc::Plain => Vec::new(),
            Enc::Strip => encoding(0, 1, 3, &s),
            Enc::Zlib => encoding(0, 3, 0, &[]),
            Enc::StripZlib => [encoding(0, 1, 3, &s), encoding(1, 1, 0, &[])].concat(),
        };
        if !encs.is_empty() {
            body.extend(el(CONTENT_ENCODINGS, &encs));
        }
        entries.extend(el(TRACK_ENTRY, &body));
        strip.push((n, s));
    }
    let stored_as = |enc: Enc, n: u64, f: &[u8]| -> Vec<u8> {
        let s = &strip.iter().find(|x| x.0 == n).unwrap().1;
        match enc {
            Enc::Plain => f.to_vec(),
            Enc::Strip => f[s.len()..].to_vec(),
            Enc::Zlib => zlib(f),
            Enc::StripZlib => zlib(&f[s.len()..]),
        }
    };
    let stored = |n: u64, f: &[u8]| stored_as(enc, n, f);
    // Fixed lacing takes frames of one size, in every coding (the files compared are laced
    // alike).
    let same_size = |n: u64, a: &[u8], b: &[u8]| {
        [Enc::Plain, Enc::Strip, Enc::Zlib, Enc::StripZlib]
            .into_iter()
            .all(|e| stored_as(e, n, a).len() == stored_as(e, n, b).len())
    };
    // Blocks: the first frame (track, time and flags), the lacing, the frames as stored.
    let mut blocks: Vec<(u64, Frame, u8, Vec<Vec<u8>>)> = Vec::new();
    let mut i = 0;
    while i < frames.len() {
        let (n, f) = (frames[i].0, frames[i].1);
        let first = stored(n, raw(&f));
        let mut group = vec![first];
        if laced.contains(&n) && lace != Lace::None {
            while group.len() < 4
                && let Some((m, g)) = frames.get(i + group.len())
                && *m == n
            {
                if lace == Lace::Fixed && !same_size(n, raw(&f), raw(g)) {
                    break;
                }
                group.push(stored(n, raw(g)));
            }
        }
        let bits = if group.len() > 1 {
            match lace {
                Lace::Xiph => 1,
                Lace::Fixed => 2,
                Lace::Ebml => 3,
                Lace::None => 0,
            }
        } else {
            0
        };
        i += group.len();
        blocks.push((n, f, bits, group));
    }
    let mut body = Vec::new();
    let mut k = 0;
    while k < blocks.len() {
        let base = blocks[k].1.ticks;
        assert!(base >= 0);
        let mut c = uel(CLUSTER_TIMESTAMP, base as u64);
        while k < blocks.len() && (blocks[k].1.ticks - base).abs() < 30_000 {
            let (n, f, bits, group) = &blocks[k];
            let mut b = vint(*n);
            b.extend_from_slice(&((f.ticks - base) as i16).to_be_bytes());
            let key = f.key.unwrap_or(false);
            b.push(u8::from(key) << 7 | u8::from(f.invisible) << 3 | bits << 1);
            if *bits != 0 {
                b.push((group.len() - 1) as u8);
                let sizes: Vec<usize> = group.iter().map(Vec::len).collect();
                match bits {
                    1 => {
                        for &s in &sizes[..sizes.len() - 1] {
                            b.extend(std::iter::repeat_n(255u8, s / 255));
                            b.push((s % 255) as u8);
                        }
                    }
                    3 => {
                        b.extend(vint(sizes[0] as u64));
                        for w in sizes[..sizes.len() - 1].windows(2) {
                            b.extend(svint(w[1] as i64 - w[0] as i64));
                        }
                    }
                    _ => {}
                }
            }
            for g in group {
                b.extend_from_slice(g);
            }
            c.extend(el(SIMPLE_BLOCK, &b));
            k += 1;
        }
        body.extend(el(CLUSTER, &c));
    }
    let seg_body = [info, el(TRACKS, &entries), body].concat();
    out.extend(el(SEGMENT, &seg_body));
    out
}

/// Where the child element `k` of `parent` starts (its ID), for copying it whole.
fn start_of(data: &[u8], parent: &El, k: &El) -> usize {
    let mut p = parent.body;
    loop {
        let e = element(data, p).unwrap();
        if e.body == k.body {
            return p;
        }
        p = e.end;
    }
}

/// Every sample of both tracks of `b` against `a`'s: times, flags and readable bytes.
/// Returns the mismatches and the samples compared.
fn compare_samples(a: &[u8], b: &[u8]) -> (usize, usize) {
    let (ma, mb) = (parse(a, true).unwrap(), parse(b, true).unwrap());
    let mut bad = 0;
    let mut n = 0;
    let mut cmp = |sa: &[Sample],
                   sb: &[Sample],
                   read_a: &dyn Fn(&Sample) -> Vec<u8>,
                   read_b: &dyn Fn(&Sample) -> Vec<u8>| {
        assert_eq!(sa.len(), sb.len());
        for (x, y) in sa.iter().zip(sb) {
            n += 1;
            let same = (x.cts, x.dts, x.duration, x.sync) == (y.cts, y.dts, y.duration, y.sync)
                && read_a(x) == read_b(y);
            bad += usize::from(!same);
        }
    };
    if let (Some(va), Some(vb)) = (&ma.video, &mb.video) {
        assert_eq!(va.codec, vb.codec);
        assert_eq!(va.config, vb.config);
        assert!(matches!(vb.layout, Layout::Mkv(_)));
        cmp(
            &va.samples,
            &vb.samples,
            &|s| va.sample_data(a, s).unwrap().into_owned(),
            &|s| vb.sample_data(b, s).unwrap().into_owned(),
        );
    }
    if let (Some(xa), Some(xb)) = (&ma.audio, &mb.audio) {
        assert_eq!(xa.codec, xb.codec);
        assert!(matches!(xb.layout, Layout::Mkv(_)));
        cmp(
            &xa.samples,
            &xb.samples,
            &|s| xa.sample_data(a, s).unwrap().into_owned(),
            &|s| xb.sample_data(b, s).unwrap().into_owned(),
        );
    }
    (bad, n)
}

fn tracks(m: &Movie) -> (bool, bool) {
    (m.video.is_some(), m.audio.is_some())
}

const FIXTURES: [&str; 9] = [
    "h264-aac.mkv",
    "h264-opus.mkv",
    "hevc.mkv",
    "mpeg2.mkv",
    "mp3-mkv.mkv",
    "dts-mkv.mkv",
    "vp8-ac3.mkv",
    "snd-mka.mka",
    "alac-mono.mkv",
];

#[test]
fn every_coding_and_lacing_gives_the_same_samples() {
    for name in FIXTURES {
        let src = fixture(name);
        let m = parse(&src, false).unwrap();
        // Sound tracks are laced, as mkvmerge does.
        let laced: Vec<u64> = children_tracks(&src)
            .into_iter()
            .filter(|t| t.kind == 2)
            .map(|t| t.number)
            .collect();
        for lace in [Lace::None, Lace::Xiph, Lace::Fixed, Lace::Ebml] {
            let plain = remux(&src, Enc::Plain, lace, &laced);
            assert_eq!(tracks(&parse(&plain, false).unwrap()), tracks(&m), "{name}");
            for enc in [Enc::Strip, Enc::Zlib, Enc::StripZlib] {
                let coded = remux(&src, enc, lace, &laced);
                let (bad, n) = compare_samples(&plain, &coded);
                eprintln!("{name} {enc:?} {lace:?}: {bad} of {n} samples differ");
                assert_eq!(bad, 0, "{name} {enc:?} {lace:?}");
                assert!(n > 0, "{name}");
            }
        }
    }
}

fn children_tracks(src: &[u8]) -> Vec<Entry> {
    let header = element(src, 0).unwrap();
    let seg = element(src, header.end).unwrap();
    let t = children(src, seg.body, seg.end)
        .find(|e| e.id == TRACKS)
        .unwrap();
    children(src, t.body, t.end)
        .filter(|c| c.id == TRACK_ENTRY)
        .map(|c| track_entry(src, &c))
        .collect()
}

/// How many bytes header stripping takes from each frame of the track of `kind`.
fn stripped(src: &[u8], kind: u64) -> usize {
    let header = element(src, 0).unwrap();
    let seg = element(src, header.end).unwrap();
    let n = children_tracks(src)
        .iter()
        .find(|t| t.kind == kind)
        .unwrap()
        .number;
    let mut frames = Vec::new();
    for e in children(src, seg.body, seg.end).filter(|e| e.id == CLUSTER) {
        cluster(src, e.body, e.end, e.unknown, &[n], &mut frames);
    }
    let fs: Vec<&[u8]> = frames
        .iter()
        .map(|f| &src[f.1.offset..f.1.offset + f.1.size])
        .collect();
    common_prefix(&fs).len()
}

fn session_of(bytes: Vec<u8>, name: &str) -> crate::session::VideoSession {
    let src = std::sync::Arc::new(Source::from_vec(name, bytes));
    crate::session::open(src, Format::Matroska).unwrap()
}

/// Every frame's planes, in order, then each again after a seek to it, and the gallery's
/// thumbnail.
fn decoded(bytes: Vec<u8>, name: &str) -> (Vec<Vec<u8>>, Vec<Vec<u8>>, Vec<u8>) {
    let s = session_of(bytes, name);
    assert_eq!(s.video().unwrap().unplayable, None, "{name}");
    let cancel = CancelToken::new();
    let planes = |r: &specular_core::Raster| -> Vec<u8> {
        r.planes
            .iter()
            .flat_map(|p| p.data.iter().copied())
            .collect()
    };
    let mut f = s.video_frames().unwrap();
    f.set_deinterlace(false);
    let mut all = Vec::new();
    while let Some(x) = f.next_frame(&cancel) {
        all.push(planes(&x.unwrap().raster));
    }
    let n = all.len() as u32;
    let mut sought = Vec::new();
    for i in [n - 1, 0, n / 2, 3.min(n - 1)] {
        assert!(f.seek(i).unwrap());
        sought.push(planes(&f.next_frame(&cancel).unwrap().unwrap().raster));
    }
    let thumb = s.decode_sampled(256, &cancel).unwrap().unwrap();
    (all, sought, planes(&thumb))
}

#[test]
fn compressed_video_decodes_bit_for_bit() {
    for name in ["h264-aac.mkv", "mpeg2.mkv", "hevc.mkv"] {
        let src = fixture(name);
        assert!(
            stripped(&src, 1) > 0,
            "{name}: header stripping has nothing to take"
        );
        let plain = decoded(remux(&src, Enc::Plain, Lace::None, &[]), name);
        assert!(plain.0.len() > 4, "{name}");
        for enc in [Enc::Strip, Enc::Zlib, Enc::StripZlib] {
            let got = decoded(remux(&src, enc, Lace::None, &[]), name);
            let bad = plain.0.iter().zip(&got.0).filter(|(a, b)| a != b).count()
                + plain.0.len().abs_diff(got.0.len());
            eprintln!(
                "{name} {enc:?}: {bad} of {} frames differ; seeks and thumbnail {}",
                plain.0.len(),
                if plain.1 == got.1 && plain.2 == got.2 {
                    "the same"
                } else {
                    "differ"
                }
            );
            assert_eq!(bad, 0, "{name} {enc:?}");
            assert_eq!(plain.1, got.1, "{name} {enc:?}: seeks");
            assert_eq!(plain.2, got.2, "{name} {enc:?}: thumbnail");
        }
    }
}

#[test]
fn compressed_sound_decodes_bit_for_bit() {
    for name in [
        "mp3-mkv.mkv",
        "vp8-ac3.mkv",
        "dts-mkv.mkv",
        "h264-aac.mkv",
        "h264-opus.mkv",
        "snd-mka.mka",
    ] {
        let src = fixture(name);
        eprintln!(
            "{name}: {} bytes stripped per sound frame",
            stripped(&src, 2)
        );
        let laced: Vec<u64> = children_tracks(&src)
            .into_iter()
            .filter(|t| t.kind == 2)
            .map(|t| t.number)
            .collect();
        let pcm = |bytes: Vec<u8>| {
            let m = parse(&bytes, true).unwrap();
            let src = std::sync::Arc::new(Source::from_vec(name, bytes));
            crate::audio::decode_track(m.audio.as_ref().unwrap(), &src).unwrap()
        };
        let want = pcm(remux(&src, Enc::Plain, Lace::Xiph, &laced));
        assert!(!want.0.is_empty(), "{name}");
        for enc in [Enc::Strip, Enc::Zlib, Enc::StripZlib] {
            let got = pcm(remux(&src, enc, Lace::Xiph, &laced));
            let bad = want
                .0
                .iter()
                .zip(&got.0)
                .filter(|(a, b)| a.to_bits() != b.to_bits())
                .count()
                + want.0.len().abs_diff(got.0.len());
            eprintln!(
                "{name} {enc:?}: {bad} of {} sound samples differ",
                want.0.len()
            );
            assert_eq!(bad, 0, "{name} {enc:?}");
            assert_eq!((want.1, want.2), (got.1, got.2), "{name} {enc:?}");
        }
    }
}

#[test]
fn refused_codings_are_named() {
    let src = fixture("h264-aac.mkv");
    // Every track stored with LZO, then encrypted.
    for (enc, video, sound) in [
        (
            encoding(0, 1, 2, &[]),
            "LZO-compressed Matroska",
            "LZO-compressed Matroska",
        ),
        (
            el(CONTENT_ENCODING, &el(CONTENT_ENCRYPTION, &[])),
            "encrypted",
            "encrypted",
        ),
        (
            encoding(0, 1, 7, &[]),
            "compressed Matroska",
            "compressed Matroska",
        ),
    ] {
        let plain = remux(&src, Enc::Plain, Lace::None, &[]);
        let f = with_encodings(&plain, &enc);
        let m = parse(&f, true).unwrap();
        assert_eq!(m.video.as_ref().unwrap().codec.unwrap().name(), video);
        assert_eq!(m.audio.as_ref().unwrap().codec.name(), sound);
        let s = session_of(f, "x.mkv");
        let v = s.video().unwrap();
        assert!(
            v.unplayable.as_deref().is_some_and(|u| u.contains(video)),
            "{:?}",
            v.unplayable
        );
    }
}

/// `f` with `ContentEncodings` holding `enc` added to every track entry.
fn with_encodings(f: &[u8], enc: &[u8]) -> Vec<u8> {
    let header = element(f, 0).unwrap();
    let seg = element(f, header.end).unwrap();
    let mut body = Vec::new();
    for e in children(f, seg.body, seg.end) {
        let at = start_of(f, &seg, &e);
        if e.id == TRACKS {
            let mut t = Vec::new();
            for c in children(f, e.body, e.end) {
                let mut b = f[c.body..c.end].to_vec();
                b.extend(el(CONTENT_ENCODINGS, enc));
                t.extend(el(TRACK_ENTRY, &b));
            }
            body.extend(el(TRACKS, &t));
        } else {
            body.extend_from_slice(&f[at..e.end]);
        }
    }
    [f[..header.end].to_vec(), el(SEGMENT, &body)].concat()
}

#[test]
fn zlib_bombs_are_bounded() {
    // A frame of 64 MB of zeros stored as about 64 kB.
    let big = zlib(&vec![0u8; 64 << 20]);
    let mut f = super::build::header("matroska");
    let entry = [
        uel(TRACK_NUMBER, 1),
        uel(TRACK_TYPE, 2),
        el(CODEC_ID, b"A_MPEG/L3"),
        el(CONTENT_ENCODINGS, &encoding(0, 1, 0, &[])),
    ]
    .concat();
    let mut block = vec![0x81, 0, 0, 0x80];
    block.extend_from_slice(&big);
    let seg = [
        el(TRACKS, &el(TRACK_ENTRY, &entry)),
        el(
            CLUSTER,
            &[uel(CLUSTER_TIMESTAMP, 0), el(SIMPLE_BLOCK, &block)].concat(),
        ),
    ]
    .concat();
    f.extend(el(SEGMENT, &seg));
    let m = parse(&f, true).unwrap();
    let a = m.audio.unwrap();
    assert_eq!(a.samples.len(), 1);
    // More than the memory budget allows: no frame, nothing allocated past the budget.
    let r = specular_core::budget::with_limit(8 << 20, || {
        a.sample_data(&f, &a.samples[0]).map(|b| b.len())
    });
    assert_eq!(r, None, "64 MB in an 8 MB budget");
    let r = specular_core::budget::with_limit(128 << 20, || {
        a.sample_data(&f, &a.samples[0]).map(|b| b.len())
    });
    assert_eq!(r, Some(64 << 20));
}
