//! Raw video clips as sessions: Apple ProRes RAW (a small movie made here, and the corpus
//! clip) and Canon Cinema RAW Light (corpus only: `tools/corpus/fetch-raw-video.sh`, skipped
//! unless `SPECULAR_RAW_VIDEO_DIR` names it).

use std::path::PathBuf;
use std::sync::Arc;

use specular_core::{CancelToken, Source};
use specular_decode::{Confidence, DecodeError, DecodeRequest, Format, Registry};

fn boxed(typ: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut b = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
    b.extend_from_slice(typ);
    b.extend_from_slice(payload);
    b
}

/// A ProRes RAW frame of `w × h` whose tiles hold no coefficients (mid-grey), of `version`.
fn frame(w: u16, h: u16, version: u8) -> Vec<u8> {
    let mut hd = vec![0u8, version];
    hd.extend_from_slice(b"test");
    hd.extend_from_slice(&w.to_be_bytes());
    hd.extend_from_slice(&h.to_be_bytes());
    hd.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
    hd.extend_from_slice(&4000u16.to_be_bytes());
    for v in [
        2.0f32, 1.5, 0.41, 0.36, 0.18, 0.21, 0.72, 0.07, 0.02, 0.12, 0.95, 1.0,
    ] {
        hd.extend_from_slice(&v.to_bits().to_be_bytes());
    }
    hd.extend_from_slice(&5600u16.to_be_bytes());
    // Flags: tile alignment 1 (two columns of 16).
    hd.extend_from_slice(&(1u16 << 1).to_be_bytes());
    let mut f = vec![0u8; 4];
    f.extend_from_slice(b"prrf");
    f.extend_from_slice(&((hd.len() + 2) as u16).to_be_bytes());
    f.extend_from_slice(&hd);
    let cols = u32::from(w).div_ceil(16);
    let n = (cols / 2 + cols % 2) * u32::from(h).div_ceil(16);
    for _ in 0..n {
        f.extend_from_slice(&8u16.to_be_bytes());
    }
    for _ in 0..n {
        f.extend_from_slice(&[8 << 3, 2, 0, 0, 0, 0, 0, 0]);
    }
    let len = f.len() as u32;
    f[..4].copy_from_slice(&len.to_be_bytes());
    f
}

/// A QuickTime movie of `frames` (25 fps), with an `aprh` sample entry.
fn movie(frames: &[Vec<u8>]) -> Vec<u8> {
    let mut out = boxed(b"ftyp", b"qt  \0\0\0\0qt  ");
    let data: Vec<u8> = frames.concat();
    let mdat_at = out.len() + 8;
    out.extend(boxed(b"mdat", &data));
    let mut offsets = Vec::new();
    let mut at = mdat_at;
    for f in frames {
        offsets.push(at as u32);
        at += f.len();
    }
    let n = frames.len() as u32;
    let mut entry = vec![0u8; 6];
    entry.extend_from_slice(&1u16.to_be_bytes());
    entry.extend_from_slice(&[0; 16]);
    entry.extend_from_slice(&64u16.to_be_bytes());
    entry.extend_from_slice(&32u16.to_be_bytes());
    entry.extend_from_slice(&[0; 50]);
    let mut stsd = vec![0, 0, 0, 0, 0, 0, 0, 1];
    stsd.extend(boxed(b"aprh", &entry));
    let mut stts = vec![0u8; 4];
    stts.extend_from_slice(&1u32.to_be_bytes());
    stts.extend_from_slice(&n.to_be_bytes());
    stts.extend_from_slice(&100u32.to_be_bytes());
    let mut stsc = vec![0u8; 4];
    for v in [1u32, 1, 1, 1] {
        stsc.extend_from_slice(&v.to_be_bytes());
    }
    let mut stsz = vec![0u8; 8];
    stsz.extend_from_slice(&n.to_be_bytes());
    for f in frames {
        stsz.extend_from_slice(&(f.len() as u32).to_be_bytes());
    }
    let mut stco = vec![0u8; 4];
    stco.extend_from_slice(&n.to_be_bytes());
    for o in offsets {
        stco.extend_from_slice(&o.to_be_bytes());
    }
    let stbl = [
        boxed(b"stsd", &stsd),
        boxed(b"stts", &stts),
        boxed(b"stsc", &stsc),
        boxed(b"stsz", &stsz),
        boxed(b"stco", &stco),
    ]
    .concat();
    let mut mdhd = vec![0u8; 12];
    mdhd.extend_from_slice(&2500u32.to_be_bytes());
    mdhd.extend_from_slice(&(100 * n).to_be_bytes());
    mdhd.extend_from_slice(&[0; 4]);
    let mut hdlr = vec![0u8; 4];
    hdlr.extend_from_slice(b"mhlr");
    hdlr.extend_from_slice(b"vide");
    hdlr.extend_from_slice(&[0; 13]);
    let minf = boxed(b"minf", &boxed(b"stbl", &stbl));
    let mdia = boxed(
        b"mdia",
        &[boxed(b"mdhd", &mdhd), boxed(b"hdlr", &hdlr), minf].concat(),
    );
    let moov = boxed(b"moov", &boxed(b"trak", &mdia));
    out.extend(moov);
    out
}

fn open(name: &str, bytes: Vec<u8>) -> specular_decode::Result<Arc<dyn specular_decode::Session>> {
    Registry::default().open(Arc::new(Source::from_vec(name, bytes)))
}

#[test]
fn prores_raw_movie_plays_its_frames() {
    let f = frame(96, 48, 0);
    let s = open("clip.mov", movie(&[f.clone(), f.clone(), f])).expect("opens");
    let info = s.info();
    assert_eq!(info.format, Format::ProResRaw);
    assert_eq!((info.width, info.height), (96, 48));
    assert!(
        info.coding
            .starts_with("Apple ProRes RAW HQ, 3 frames, 25.000 fps"),
        "{}",
        info.coding
    );
    let v = s.video().expect("movie facts");
    assert_eq!((v.frames, v.codec.as_str()), (3, "Apple ProRes RAW HQ"));
    assert_eq!(v.duration_us, Some(120_000));
    assert_eq!(s.animation().and_then(|a| a.frame_count), Some(3));
    let h = s.header().expect("header facts");
    assert!(
        h.fields
            .iter()
            .any(|(k, v)| k == "white balance" && v == "5600 K")
    );
    // The first frame, and every frame through the player's decoder.
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert_eq!((r.width, r.height), (96, 48));
    let mut frames = s.frames(2).unwrap();
    assert_eq!(frames.delays().map(|d| d.len()), Some(3));
    let mut n = 0;
    while let Some(fr) = frames.next_frame(&CancelToken::new()) {
        let fr = fr.unwrap();
        assert_eq!(fr.index, n);
        assert_eq!((fr.raster.width, fr.raster.height), (48, 24));
        n += 1;
    }
    assert_eq!(n, 3);
    assert!(frames.seek(2).unwrap());
    assert_eq!(
        frames
            .next_frame(&CancelToken::new())
            .unwrap()
            .unwrap()
            .index,
        2
    );
}

#[test]
fn other_movies_stay_movies() {
    // A QuickTime movie without a ProRes RAW track is not taken.
    let mut m = movie(&[frame(32, 32, 0)]);
    let at = m.windows(4).position(|w| w == b"aprh").unwrap();
    m[at..at + 4].copy_from_slice(b"apch");
    let c = Registry::default().identify(&Source::from_vec("a.mov", m));
    assert!(
        c.iter()
            .all(|c| c.format != Format::ProResRaw || c.confidence == Confidence::Guess),
        "{c:?}"
    );
}

#[test]
fn refusals_say_why() {
    // A frame version this decoder does not know.
    let e = open("v2.mov", movie(&[frame(32, 32, 2)]))
        .err()
        .expect("refused");
    assert!(
        matches!(&e, DecodeError::Unsupported(m) if m.contains("frame version")),
        "{e:?}"
    );
    // A movie cut before its first frame's end: the sample table's frame runs past it.
    let mut m = movie(&[frame(32, 32, 0)]);
    let z = m.windows(4).position(|w| w == b"stsz").unwrap() + 4;
    m[z + 12..z + 16].copy_from_slice(&(1u32 << 20).to_be_bytes());
    let e = open("cut.mov", m).err().expect("refused");
    assert!(
        matches!(&e, DecodeError::Unsupported(m) if m.contains("cut short")),
        "{e:?}"
    );
}

fn corpus(name: &str) -> Option<Arc<Source>> {
    let dir = std::env::var_os("SPECULAR_RAW_VIDEO_DIR")?;
    Source::open(PathBuf::from(dir).join(name))
        .ok()
        .map(Arc::new)
}

#[test]
fn corpus_clips() {
    let Some(crm) = corpus("c200_prefix.crm") else {
        eprintln!("SPECULAR_RAW_VIDEO_DIR not set; skipping");
        return;
    };
    let s = Registry::default().open(crm).expect("opens");
    let info = s.info().clone();
    eprintln!("{}: {}", info.format.name(), info.coding);
    assert_eq!(info.format, Format::CinemaRawLight);
    assert_eq!((info.width, info.height), (4096, 2160));
    assert!(
        info.coding
            .contains("10 frames of 2940 (the file is cut short)")
    );
    let v = s.video().unwrap();
    assert_eq!(v.frames, 10);
    assert_eq!(v.silent_reason.as_deref(), Some("PCM"));
    assert_eq!(s.camera().unwrap().camera, "Canon EOS C200");
    assert!(s.decode_preview().is_some());
    let mut f = s.frames(8).unwrap();
    let t = std::time::Instant::now();
    let first = f.next_frame(&CancelToken::new()).unwrap().unwrap();
    eprintln!(
        "CRM frame 0 at 1/8: {}×{} in {:.0} ms",
        first.raster.width,
        first.raster.height,
        t.elapsed().as_secs_f64() * 1e3
    );
    assert_eq!((first.raster.width, first.raster.height), (512, 270));

    let prr = corpus("fs_aprh_4f.mov").expect("ProRes RAW clip");
    let s = Registry::default().open(prr).expect("opens");
    eprintln!("{}: {}", s.info().format.name(), s.info().coding);
    assert_eq!(s.info().format, Format::ProResRaw);
    assert_eq!(s.video().unwrap().frames, 4);
    let mut f = s.frames(4).unwrap();
    let t = std::time::Instant::now();
    let mut n = 0;
    while let Some(fr) = f.next_frame(&CancelToken::new()) {
        fr.unwrap();
        n += 1;
    }
    eprintln!(
        "ProRes RAW: {n} frames at 1/4 in {:.0} ms",
        t.elapsed().as_secs_f64() * 1e3
    );
    assert_eq!(n, 4);
}
