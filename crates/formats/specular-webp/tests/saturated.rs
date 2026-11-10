//! Saturated colours in lossy WebP: VP8 codes them outside 16–235 / 16–240, so the planes stay
//! studio range and convert as libwebp does (`saturated-q90.webp`: colour bars and ramps made
//! with `cwebp -q 90 -sns 0`, the reference `saturated-q90.pam` from `dwebp -pam`, libwebp
//! 1.5.0). Expanded to full range in 8 bits they clipped, up to 25/255 off.

use specular_webp::image::to_rgba;
use specular_webp::{Pixels, WebpImage};

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

#[test]
fn saturated_colours_equal_libwebp() {
    let file = data("saturated-q90.webp");
    let img = WebpImage::parse(&file[..]).unwrap();
    for parallel in [false, true] {
        let d = img.decode(1, parallel).unwrap();
        let Pixels::YCbCr420 { cb, cr, .. } = &d.pixels else {
            panic!("planar output expected");
        };
        // Studio range kept: the bars reach the chroma extremes, 16 and 240.
        let c = cb.data.iter().chain(&cr.data);
        assert!(c.clone().any(|&v| v >= 240) && c.clone().any(|&v| v <= 16));
        let rgba = to_rgba(d);
        let Pixels::Rgba(p) = &rgba.pixels else {
            unreachable!()
        };
        let want = data("saturated-q90.pam");
        let (w, h, want) = pam(&want);
        assert_eq!((rgba.width, rgba.height), (w, h));
        let got: Vec<u8> = (0..h as usize)
            .flat_map(|y| p.data[y * p.stride..][..w as usize * 4].to_vec())
            .collect();
        // RGB as dwebp writes it (its PAM is RGBA, alpha 255). libwebp rounds its fancy
        // chroma in two steps, ours in one: a code at most, on few samples.
        let d: Vec<i32> = got
            .iter()
            .zip(want)
            .map(|(&a, &b)| (i32::from(a) - i32::from(b)).abs())
            .collect();
        let max = d.iter().copied().max().unwrap_or(0);
        let off = d.iter().filter(|&&v| v > 0).count();
        assert!(
            max <= 1 && off * 50 <= d.len(),
            "max {max}, {off} of {} off",
            d.len()
        );
    }
}
