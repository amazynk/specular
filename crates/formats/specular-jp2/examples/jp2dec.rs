//! Decode a JPEG 2000 file to PGM/PPM (PAM for alpha): `jp2dec in.jp2 out.pnm [reduce] [x,y,w,h]`.
//! 8-bit output; PPM drops alpha.

use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: jp2dec in out.pnm [reduce] [x,y,w,h] [layers]");
        std::process::exit(2);
    }
    let data = std::fs::read(&args[1]).expect("read input");
    let reduce = args.get(3).map(|s| s.parse().expect("reduce")).unwrap_or(0);
    let region = args.get(4).filter(|s| s.contains(',')).map(|s| {
        let v: Vec<u32> = s.split(',').map(|x| x.parse().expect("region")).collect();
        specular_jp2::Region {
            x: v[0],
            y: v[1],
            w: v[2],
            h: v[3],
        }
    });
    let max_layers = args.get(5).map(|s| s.parse().expect("layers"));
    let opts = specular_jp2::DecodeOptions {
        reduce,
        region,
        max_layers,
        ..Default::default()
    };
    let t = std::time::Instant::now();
    let img = match specular_jp2::decode(&data, &opts) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };
    eprintln!(
        "{}x{} {:?} truncated={} in {:.1} ms",
        img.width,
        img.height,
        img.format,
        img.truncated,
        t.elapsed().as_secs_f64() * 1e3
    );
    let specular_jp2::Pixels::U8(px) = &img.pixels else {
        unreachable!()
    };
    let mut f = std::io::BufWriter::new(std::fs::File::create(&args[2]).expect("create"));
    if img.format == specular_jp2::PixelFormat::Gray8 {
        write!(f, "P5\n{} {}\n255\n", img.width, img.height).unwrap();
        f.write_all(px).unwrap();
    } else {
        write!(f, "P6\n{} {}\n255\n", img.width, img.height).unwrap();
        for p in px.chunks_exact(4) {
            f.write_all(&p[..3]).unwrap();
        }
    }
}
