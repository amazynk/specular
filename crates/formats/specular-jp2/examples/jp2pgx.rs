//! Decode every component of a JPEG 2000 file to PGX (T.803 reference format), before any
//! colour mapping: `jp2pgx in out_prefix [reduce] [layers]` writes `out_prefix-<c>.pgx`.

use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: jp2pgx in out_prefix [reduce] [layers]");
        std::process::exit(2);
    }
    let data = std::fs::read(&args[1]).expect("read input");
    let reduce = args.get(3).map(|s| s.parse().expect("reduce")).unwrap_or(0);
    let layers = args.get(4).map(|s| s.parse().expect("layers"));
    let dec = match specular_jp2::Decoder::new(&data) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };
    let t = std::time::Instant::now();
    let comps = match dec.decode_components(reduce, layers) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };
    eprintln!(
        "{} components in {:.1} ms",
        comps.len(),
        t.elapsed().as_secs_f64() * 1e3
    );
    for (c, comp) in comps.iter().enumerate() {
        let ci = &dec.info().components[c];
        let prec = ci.precision.min(16) as u32;
        let off = if ci.signed { 1i32 << (prec - 1) } else { 0 };
        let path = format!("{}-{c}.pgx", args[2]);
        let mut f = std::io::BufWriter::new(std::fs::File::create(&path).expect("create"));
        let sign = if ci.signed { '-' } else { '+' };
        writeln!(f, "PG ML {sign}{prec} {} {}", comp.width, comp.height).unwrap();
        for &s in &comp.samples {
            let v = s as i32 - off;
            if prec <= 8 {
                f.write_all(&[v as u8]).unwrap();
            } else {
                f.write_all(&(v as u16).to_be_bytes()).unwrap();
            }
        }
    }
}
