//! Decodes an AC-3 / E-AC-3 stream to s16le (f32le with `f32`): `cargo run --example
//! ac3_decode -- IN OUT [all|stereo] [nodither] [f32]`. Prints the stream and the speed.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&args[1]).expect("input");
    let all = args.get(3).is_some_and(|a| a == "all");
    let nodither = args.iter().any(|a| a == "nodither");
    let float = args.iter().any(|a| a == "f32");
    let opts = specular_ac3::Options {
        stereo: !all,
        levels: false,
        drc: 1.0,
        dither: !nodither,
        lfe_mix: false,
    };
    let mut d = specular_ac3::Decoder::new(opts);
    let mut out = Vec::new();
    let t = std::time::Instant::now();
    if let Err(e) = d.decode(&data, &mut out) {
        eprintln!("decode: {e}");
    }
    if let Err(e) = d.flush(&mut out) {
        eprintln!("flush: {e}");
    }
    let secs = t.elapsed().as_secs_f64();
    let pcm: Vec<u8> = if float {
        out.iter().flat_map(|v| v.to_le_bytes()).collect()
    } else {
        out.iter()
            .flat_map(|v| ((v * 32768.0).round().clamp(-32768.0, 32767.0) as i16).to_le_bytes())
            .collect()
    };
    std::fs::write(&args[2], pcm).expect("output");
    let info = d.info();
    let ch = d.channels().max(1);
    let rate = info.map_or(48_000, |i| i.sample_rate);
    let media = out.len() as f64 / ch as f64 / f64::from(rate);
    eprintln!(
        "{:?}\n{:?}\n{} samples, {media:.2} s in {secs:.3} s: {:.0}x real time",
        specular_ac3::probe(&data),
        info,
        out.len(),
        media / secs.max(1e-9)
    );
}
