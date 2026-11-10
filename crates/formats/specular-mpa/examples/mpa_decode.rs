//! Decodes an MPEG audio Layer I/II stream to s16le: `cargo run --example mpa_decode -- IN OUT`.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&args[1]).expect("input");
    let mut d = specular_mpa::Decoder::new();
    let mut out = Vec::new();
    d.decode(&data, &mut out).expect("decode");
    d.flush(&mut out).expect("decode");
    let pcm: Vec<u8> = out
        .iter()
        .flat_map(|v| ((v * 32768.0).round().clamp(-32768.0, 32767.0) as i16).to_le_bytes())
        .collect();
    std::fs::write(&args[2], pcm).expect("output");
    eprintln!("{:?}, {} samples", d.header(), out.len());
}
