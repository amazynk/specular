//! Matroska turns and mirrors (`Projection`: yaw 180 mirrors, then the roll turns), against
//! ffmpeg: the frame as stored, turned by the orientation read here, is ffmpeg's own
//! turned-for-display decode (tests/data/gen_fixtures_mkvflip.sh), pixel for pixel.

use std::path::{Path, PathBuf};

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

/// Where stored pixel (x, y) of a w×h picture lands on screen under EXIF orientation `o`.
fn exif(o: u8, w: usize, h: usize, x: usize, y: usize) -> (usize, usize) {
    match o {
        1 => (x, y),
        2 => (w - 1 - x, y),
        3 => (w - 1 - x, h - 1 - y),
        4 => (x, h - 1 - y),
        5 => (y, x),
        6 => (h - 1 - y, x),
        7 => (h - 1 - y, w - 1 - x),
        _ => (y, w - 1 - x),
    }
}

#[test]
fn turns_and_mirrors_match_ffmpeg() {
    let (w, h) = (64, 32);
    let base = std::fs::read(data("flip-base.gray")).unwrap();
    let mut seen = Vec::new();
    for r in [0, 90, 180, 270] {
        for m in ["", "h"] {
            let name = format!("flip-r{r}{m}");
            let bytes = std::fs::read(data(&format!("{name}.mkv"))).unwrap();
            let v = specular_video::parse_movie_with(&bytes, false, None)
                .unwrap()
                .video
                .unwrap();
            assert_eq!(v.mirror, m == "h", "{name}");
            let o = v.orientation();
            let want = std::fs::read(data(&format!("{name}.gray"))).unwrap();
            let sw = if o >= 5 { h } else { w };
            let mut mismatches = 0;
            for y in 0..h {
                for x in 0..w {
                    let (sx, sy) = exif(o, w, h, x, y);
                    mismatches += usize::from(base[y * w + x] != want[sy * sw + sx]);
                }
            }
            assert_eq!(mismatches, 0, "{name}: orientation {o}");
            seen.push(o);
        }
    }
    seen.sort_unstable();
    assert_eq!(seen, [1, 2, 3, 4, 5, 6, 7, 8]);
}
