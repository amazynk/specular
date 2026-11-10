//! Compressed Matroska from mkvmerge (tools/corpus/mkv-compressed.sh): zlib-compressed tracks,
//! laced as mkvmerge laces, against the same files remuxed without compression. Every frame, every frame after a seek, the thumbnail and the sound
//! must be the same, bit for bit. Skips itself when the set has not been generated.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::{CancelToken, Raster, Source};
use specular_decode::{Format, Frames, Session};
use specular_video::{audio, session};

fn dir() -> Option<PathBuf> {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/out/mkv-compressed");
    if d.join("manifest.tsv").is_file() {
        Some(d)
    } else {
        eprintln!("skipped: run tools/corpus/mkv-compressed.sh");
        None
    }
}

fn planes(r: &Raster) -> Vec<u8> {
    r.planes
        .iter()
        .flat_map(|p| p.data.iter().copied())
        .collect()
}

/// What a file plays: every frame, frames after seeks, the thumbnail, and its sound.
#[derive(PartialEq)]
struct Played {
    frames: Vec<Vec<u8>>,
    sought: Vec<Vec<u8>>,
    thumb: Vec<u8>,
    sound: Option<(Vec<u32>, u16, u32)>,
}

fn play(path: &Path) -> Played {
    let src = Arc::new(Source::open(path).unwrap());
    let movie = specular_video::parse_movie(src.bytes(), true).unwrap();
    let sound = movie.audio.as_ref().map(|a| {
        let (pcm, ch, rate) = audio::decode_track(a, &src).unwrap();
        (pcm.iter().map(|v| v.to_bits()).collect(), ch, rate)
    });
    let (mut frames, mut sought, mut thumb) = (Vec::new(), Vec::new(), Vec::new());
    if movie.video.is_some() {
        let s = session::open(src.clone(), Format::Matroska).unwrap();
        let name = path.display();
        assert_eq!(s.video().unwrap().unplayable, None, "{name}");
        let cancel = CancelToken::new();
        let mut f = s.video_frames().unwrap();
        f.set_deinterlace(false);
        while let Some(x) = f.next_frame(&cancel) {
            frames.push(planes(&x.unwrap().raster));
        }
        let n = frames.len() as u32;
        for i in [n - 1, 0, n / 2, 1.min(n - 1)] {
            assert!(f.seek(i).unwrap());
            sought.push(planes(&f.next_frame(&cancel).unwrap().unwrap().raster));
        }
        thumb = planes(&s.decode_sampled(256, &cancel).unwrap().unwrap());
    }
    Played {
        frames,
        sought,
        thumb,
        sound,
    }
}

#[test]
fn mkvmerge_compressed_tracks_play_as_the_plain_ones() {
    let Some(d) = dir() else { return };
    let manifest = std::fs::read_to_string(d.join("manifest.tsv")).unwrap();
    let mut checked = 0;
    for line in manifest.lines() {
        let name = line.trim();
        let want = play(&d.join(format!("{name}-none.mkv")));
        {
            let k = "zlib";
            let got = play(&d.join(format!("{name}-{k}.mkv")));
            let bad_frames = want
                .frames
                .iter()
                .zip(&got.frames)
                .filter(|(a, b)| a != b)
                .count()
                + want.frames.len().abs_diff(got.frames.len());
            let bad_sound = match (&want.sound, &got.sound) {
                (Some(a), Some(b)) => {
                    a.0.iter().zip(&b.0).filter(|(x, y)| x != y).count()
                        + a.0.len().abs_diff(b.0.len())
                }
                (None, None) => 0,
                _ => usize::MAX,
            };
            eprintln!(
                "{name}-{k}: {bad_frames} of {} frames, {bad_sound} of {} sound samples differ",
                want.frames.len(),
                want.sound.as_ref().map_or(0, |s| s.0.len())
            );
            assert_eq!((bad_frames, bad_sound), (0, 0), "{name}-{k}");
            assert!(want == got, "{name}-{k}: seeks, thumbnail or sound format");
            checked += 1;
        }
    }
    assert!(checked >= 14);
}
