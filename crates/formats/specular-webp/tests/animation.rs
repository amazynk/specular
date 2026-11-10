//! Animated WebP: composited frames vs libwebp's `WebPAnimDecoder` (frames dumped with
//! `anim_dump -pam`, stored next to the files).

use specular_webp::{Animation, Pixels, WebpImage};

fn data(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn pam(bytes: &[u8]) -> (u32, u32, &[u8]) {
    let end = bytes.windows(7).position(|w| w == b"ENDHDR\n").unwrap() + 7;
    let text = std::str::from_utf8(&bytes[..end]).unwrap();
    let field = |k: &str| -> u32 {
        let at = text.find(k).unwrap() + k.len();
        text[at..]
            .split_whitespace()
            .next()
            .unwrap()
            .parse()
            .unwrap()
    };
    (field("WIDTH"), field("HEIGHT"), &bytes[end..])
}

fn check(name: &str, frames: usize, loop_count: u32, durations: &[u32], tol: i32) {
    let file = data(&format!("{name}.webp"));
    let img = WebpImage::parse(&file[..]).unwrap();
    let info = img.info();
    assert!(info.animated);
    assert_eq!(info.frame_count as usize, frames);
    assert_eq!(info.loop_count, loop_count);
    let mut anim = Animation::new(&file[..]).unwrap();
    assert_eq!(durations.len(), frames);
    for pass in 0..2 {
        for (i, &duration) in durations.iter().enumerate() {
            let f = anim.next_frame().unwrap().unwrap();
            assert_eq!(f.duration_ms, duration, "{name} frame {i}");
            let want = data(&format!("{name}.{i}.pam"));
            let (w, h, want) = pam(&want);
            assert_eq!((f.image.width, f.image.height), (w, h));
            let Pixels::Rgba(p) = &f.image.pixels else {
                panic!("expected RGBA")
            };
            let mut worst = 0;
            for y in 0..h as usize {
                let got = &p.data[y * p.stride..][..w as usize * 4];
                let exp = &want[y * w as usize * 4..][..w as usize * 4];
                for (g, e) in got.chunks_exact(4).zip(exp.chunks_exact(4)) {
                    assert_eq!(
                        g[3], e[3],
                        "{name} frame {i} alpha at row {y} (pass {pass})"
                    );
                    if e[3] != 0 {
                        if std::env::var_os("ANIM_DEBUG").is_some() && g[..3] != e[..3] {
                            eprintln!("{name} f{i} y{y}: got {g:?} want {e:?}");
                        }
                        for c in 0..3 {
                            worst = worst.max((i32::from(g[c]) - i32::from(e[c])).abs());
                        }
                    }
                }
            }
            assert!(
                worst <= tol,
                "{name} frame {i}: max colour error {worst} > {tol}"
            );
        }
        assert!(anim.next_frame().is_none());
        anim.rewind();
    }
}

#[test]
fn lossless_animation_matches_libwebp() {
    check("anim-lossless", 3, 3, &[100, 50, 70], 0);
}

#[test]
fn mixed_animation_matches_libwebp() {
    check("anim-mixed", 4, 0, &[40, 40, 40, 40], 0);
}

#[test]
fn lossy_animation_matches_libwebp() {
    // Lossy frames: chroma upsampling rounding differs slightly from libwebp.
    check("anim-lossy", 3, 0, &[30, 30, 30], 3);
}

#[test]
fn truncated_animation_never_panics() {
    for name in ["anim-lossless.webp", "anim-mixed.webp", "anim-lossy.webp"] {
        let file = data(name);
        for len in 0..file.len() {
            let part = &file[..len];
            if let Ok(img) = WebpImage::parse(part) {
                let _ = img.decode(1, true);
            }
            if let Ok(mut a) = Animation::new(part) {
                for _ in 0..8 {
                    match a.next_frame() {
                        Some(Ok(_)) => {}
                        _ => break,
                    }
                }
            }
        }
    }
}
