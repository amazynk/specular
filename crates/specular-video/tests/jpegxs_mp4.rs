//! JPEG XS in MP4 (`jxsm`): a movie written by `tools/corpus/jpegxs-wrap.py mp4` (an
//! independent writer of the ISO/IEC 21122-3 sample entry; FFmpeg's ffprobe reads its track
//! as `jxsm`, 64×32, 2 frames at 25 fps) through the MP4 reader and the player's decoder;
//! and, with `SPECULAR_JPEGXS_CORPUS` set (tools/corpus/fetch-jpegxs.sh), the TDC sequences
//! muxed into movies against the ISO/IEC 21122-5 reference decoder:
//!
//! `SPECULAR_JPEGXS_CORPUS=… cargo test -p specular-video --test jpegxs_mp4 -- --include-ignored --nocapture`

use sha2::{Digest, Sha256};
use specular_video::mp4::{self, Codec};
use specular_video::vpx::Vpx;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../formats/specular-jpegxs/tests/fixtures")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

#[test]
fn jxsm_movie_plays_its_codestreams() {
    let m = fixture("main-2frames.mp4");
    let parsed = mp4::parse(&m, true).unwrap();
    let v = parsed.video.expect("a video track");
    assert_eq!(v.codec, Some(Codec::JpegXs));
    assert_eq!((v.width, v.height), (64, 32));
    assert_eq!(v.samples.len(), 2);
    assert_eq!(v.duration_us(), Some(80_000));
    let want = [fixture("main422-10.jxs"), fixture("main422-10-6bpp.jxs")];
    let mut dec = Vpx::new(Codec::JpegXs, &v.config).unwrap();
    for (s, w) in v.samples.iter().zip(&want) {
        let bytes = v.sample_data(&m, s).unwrap();
        assert_eq!(&*bytes, w.as_slice());
        let f = dec.decode(&bytes).unwrap().expect("a picture");
        assert_eq!((f.width, f.height, f.bit_depth), (64, 32, 10));
    }
}

#[test]
#[ignore]
fn corpus_movies_match_reference_decoder() {
    let Some(root) = std::env::var_os("SPECULAR_JPEGXS_CORPUS") else {
        eprintln!("SPECULAR_JPEGXS_CORPUS not set");
        return;
    };
    let dir = Path::new(&root).join("mp4");
    let Ok(text) = std::fs::read_to_string(dir.join("manifest.tsv")) else {
        eprintln!("no mp4/manifest.tsv (run tools/corpus/jpegxs-wrap.py corpus)");
        return;
    };
    // FILE.mp4#N: frame N with its sample count and SHA-256.
    type Frames = Vec<(usize, usize, String)>;
    let mut movies: Vec<(String, Frames)> = Vec::new();
    for l in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
    {
        let c: Vec<&str> = l.split('\t').collect();
        let (file, n) = c[0].split_once('#').unwrap();
        if movies.last().is_none_or(|m| m.0 != file) {
            movies.push((file.to_owned(), Vec::new()));
        }
        let frames = &mut movies.last_mut().unwrap().1;
        frames.push((n.parse().unwrap(), c[2].parse().unwrap(), c[3].to_owned()));
    }
    let (mut same, mut differ) = (0, 0);
    for (file, frames) in &movies {
        let m = std::fs::read(dir.join(file)).unwrap();
        let v = mp4::parse(&m, true).unwrap().video.unwrap();
        assert_eq!(v.codec, Some(Codec::JpegXs), "{file}");
        assert_eq!(v.samples.len(), frames.len(), "{file}");
        // One decoder per movie: TDC frames predict from the previous ones.
        let mut seq = specular_jpegxs::Sequence::new();
        for (n, count, sha) in frames {
            let bytes = v.sample_data(&m, &v.samples[*n]).unwrap();
            let ok = seq.decode(&bytes).is_ok_and(|s| {
                let ours: Vec<u8> = s
                    .planes
                    .iter()
                    .flat_map(|p| p.data.iter().flat_map(|x| x.to_le_bytes()))
                    .collect();
                !s.truncated
                    && ours.len() / 2 == *count
                    && format!("{:x}", Sha256::digest(&ours)) == *sha
            });
            if ok {
                same += 1;
            } else {
                differ += 1;
            }
            let status = if ok { "pass" } else { "fail" };
            println!("ORACLE\tJXS/MP4 jxsm\t{file}#{n}\t{status}");
        }
    }
    eprintln!(
        "JPEG XS in MP4: {} movies, {same} frames identical to the reference decoder, {differ} differing",
        movies.len()
    );
    assert_eq!(differ, 0);
}
