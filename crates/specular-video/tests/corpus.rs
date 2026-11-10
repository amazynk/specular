//! The demuxer and detection on real encoder output: tools/corpus/video-refs.sh (ffmpeg with
//! libx264 / libx265). Skips itself when the set has not been generated.

use std::path::{Path, PathBuf};

use specular_video::{Kind, Source, mp4};

fn refs() -> Option<PathBuf> {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/out/video-ref");
    if d.join("manifest.tsv").is_file() {
        Some(d)
    } else {
        eprintln!("skipped: run tools/corpus/video-refs.sh");
        None
    }
}

/// (name, codec, width, height, frames)
fn manifest(d: &Path) -> Vec<(String, String, u32, u32, u32)> {
    std::fs::read_to_string(d.join("manifest.tsv"))
        .unwrap()
        .lines()
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (
                f[0].to_owned(),
                f[1].to_owned(),
                f[2].parse().unwrap(),
                f[3].parse().unwrap(),
                f[4].parse().unwrap(),
            )
        })
        .collect()
}

fn clip_path(d: &Path, name: &str) -> PathBuf {
    let mp4 = d.join("clips").join(format!("{name}.mp4"));
    if mp4.is_file() {
        mp4
    } else {
        d.join("clips").join(format!("{name}.mov"))
    }
}

#[test]
fn demuxes_every_reference_clip() {
    let Some(d) = refs() else { return };
    for (name, codec, w, h, frames) in manifest(&d) {
        let data = std::fs::read(clip_path(&d, &name)).unwrap();
        let m = mp4::parse(&data, true).unwrap_or_else(|e| panic!("{name}: {e}"));
        let v = m.video.as_ref().unwrap();
        let want = match codec.as_str() {
            "h264" => mp4::Codec::H264,
            "hevc" => mp4::Codec::Hevc,
            c => panic!("{c}"),
        };
        assert_eq!(v.codec, Some(want), "{name}");
        assert_eq!((v.width, v.height), (w, h), "{name}");
        // The JVT conformance clips store one field per sample where they code fields, at
        // libavcodec's 25 fps for raw H.264; the others are a frame per sample at 30 fps.
        let jvt = name.starts_with("h264-jvt-");
        if jvt {
            assert!(v.samples.len() as u32 >= frames, "{name}");
        } else {
            assert_eq!(v.samples.len() as u32, frames, "{name}");
        }
        assert!(v.samples[0].sync, "{name}: first sample is a sync sample");
        assert!(!v.config.is_empty(), "{name}");
        // Every sample lies in the file and starts with a length-prefixed NAL unit.
        for s in &v.samples {
            let b = mp4::sample_bytes(&data, s).unwrap();
            let n = u32::from_be_bytes(b[..4].try_into().unwrap()) as usize;
            assert!(n + 4 <= b.len(), "{name}: NAL length");
        }
        // Presentation times are distinct, the frames on a regular grid.
        let mut cts: Vec<i64> = v.samples.iter().map(|s| s.cts).collect();
        cts.sort_unstable();
        cts.dedup();
        assert_eq!(
            cts.len(),
            v.samples.len(),
            "{name}: distinct presentation times"
        );
        let d_us = m.duration_us().unwrap();
        let want_us = frames as u64 * 1_000_000 / if jvt { 25 } else { 30 };
        assert!(d_us.abs_diff(want_us) < 40_000, "{name}: {d_us} µs");
    }
}

#[test]
fn rotation_and_content_identifier() {
    let Some(d) = refs() else { return };
    let rot = std::fs::read(d.join("clips/h264-rot90.mp4")).unwrap();
    let m = mp4::parse(&rot, false).unwrap();
    assert_eq!(m.video.as_ref().unwrap().rotation, 90);
    let live = std::fs::read(d.join("clips/live.mov")).unwrap();
    let m = mp4::parse(&live, false).unwrap();
    assert_eq!(
        m.content_id.as_deref(),
        Some("5C1F2E8A-0D6B-4C2B-9E7A-3F1D2B6C8E90")
    );
}

fn xmp_of_jpeg(f: &[u8]) -> Option<String> {
    let ns = b"http://ns.adobe.com/xap/1.0/\0";
    let mut p = 2;
    while p + 4 <= f.len() && f[p] == 0xff {
        let len = u16::from_be_bytes([f[p + 2], f[p + 3]]) as usize;
        let body = &f[p + 4..p + 2 + len];
        if f[p + 1] == 0xe1 && body.starts_with(ns) {
            return Some(String::from_utf8_lossy(&body[ns.len()..]).into_owned());
        }
        if f[p + 1] == 0xda {
            break;
        }
        p += 2 + len;
    }
    None
}

#[test]
fn detects_every_motion_still() {
    let Some(d) = refs() else { return };
    let m = d.join("motion");
    let cases: &[(&str, Kind, &str)] = &[
        ("PXL_motion.MP.jpg", Kind::MotionPhoto, "H.264"),
        ("PXL_motion_hevc.MP.jpg", Kind::MotionPhoto, "HEVC"),
        ("MVIMG_micro.jpg", Kind::MicroVideo, "H.264"),
        ("SM_motion.jpg", Kind::Samsung, "HEVC"),
        ("PXL_heic.MP.heic", Kind::MotionPhoto, "HEVC"),
    ];
    for &(name, kind, codec) in cases {
        let p = m.join(name);
        if !p.is_file() {
            eprintln!("skipped {name}");
            continue;
        }
        let f = std::fs::read(&p).unwrap();
        let xmp = xmp_of_jpeg(&f);
        let t = std::time::Instant::now();
        let info = specular_video::probe_embedded(&f, xmp.as_deref())
            .unwrap_or_else(|| panic!("{name}: not detected"));
        let took = t.elapsed();
        eprintln!("{name}: {:?} in {took:?}", info.kind);
        assert_eq!(info.kind, kind, "{name}");
        assert_eq!(info.codec.as_deref(), Some(codec), "{name}");
        assert!(matches!(info.source, Source::Embedded(_)));
        assert!(info.frames > 0 && info.duration_us.is_some(), "{name}");
    }
    // The plain stills are not motion photos.
    for name in ["still.jpg", "still.heic"] {
        let p = m.join(name);
        if let Ok(f) = std::fs::read(&p) {
            assert_eq!(specular_video::probe_embedded(&f, None), None, "{name}");
        }
    }
    // The Live Photo pair, by name (the synthetic JPEG has no Apple MakerNote).
    let info = specular_video::probe_live(&m.join("IMG_0001.JPG"), None).unwrap();
    assert_eq!(info.kind, Kind::LivePhoto);
    assert_eq!(info.paired, None);
    assert_eq!((info.width, info.height, info.frames), (480, 360, 45));
    assert_eq!(specular_video::probe_live(&m.join("still.jpg"), None), None);
}
