//! Decode with the `hayro-jpeg2000` oracle and write PGM/PPM (alpha dropped).
use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: hayrodec in out.pnm");
        std::process::exit(2);
    }
    let data = std::fs::read(&args[1]).expect("read");
    let t = std::time::Instant::now();
    let img = hayro_jpeg2000::Image::new(&data, &hayro_jpeg2000::DecodeSettings::default())
        .expect("parse");
    let px = img.decode().expect("decode");
    let el = t.elapsed();
    let (w, h) = (img.width() as usize, img.height() as usize);
    let nc = img.color_space().num_channels() as usize + usize::from(img.has_alpha());
    let gray = nc <= 2;
    let mut f = std::fs::File::create(&args[2]).expect("create");
    write!(f, "{}\n{w} {h}\n255\n", if gray { "P5" } else { "P6" }).unwrap();
    let mut out = Vec::with_capacity(w * h * 3);
    for p in px.chunks_exact(nc) {
        if gray {
            out.push(p[0]);
        } else {
            out.extend_from_slice(&p[..3]);
        }
    }
    f.write_all(&out).unwrap();
    println!("{w}x{h} nc={nc} in {:.1} ms", el.as_secs_f64() * 1e3);
}
