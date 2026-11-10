//! Dump the decoded pixels of a WebP for differential checks against `dwebp`:
//! planar Y, Cb, Cr (studio range, as [`specular_webp::Pixels::YCbCr420`]) or interleaved RGBA.
//! `cargo run --release -p specular-webp --example webp_dump -- IN OUT [parallel]`
fn main() {
    let mut args = std::env::args().skip(1);
    let input = args.next().expect("input");
    let output = args.next().expect("output");
    let parallel = args.next().is_some_and(|a| a == "parallel");
    let data = std::fs::read(&input).unwrap();
    let img = specular_webp::WebpImage::parse(&data[..]).unwrap();
    let d = img.decode(1, parallel).unwrap();
    let mut out = Vec::new();
    let mut put = |p: &specular_webp::Plane, bpp: usize| {
        for y in 0..p.height as usize {
            out.extend_from_slice(&p.data[y * p.stride..][..p.width as usize * bpp]);
        }
    };
    let kind = match &d.pixels {
        specular_webp::Pixels::YCbCr420 { y, cb, cr } => {
            put(y, 1);
            put(cb, 1);
            put(cr, 1);
            "yuv"
        }
        specular_webp::Pixels::Rgba(p) => {
            put(p, 4);
            "rgba"
        }
    };
    std::fs::write(&output, out).unwrap();
    println!("{kind} {} {}", d.width, d.height);
}
