//! Decodes an Ogg Vorbis file to s16le: `cargo run --example vorbis_decode -- IN.ogg OUT.s16`.
//! Prints the decode speed as a multiple of real time.

#[path = "../tests/ogg/mod.rs"]
mod ogg;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let file = std::fs::read(&args[1]).expect("input");
    let packets = ogg::packets(&file);
    let start = std::time::Instant::now();
    let mut d = specular_vorbis::Decoder::new(&packets[0].data, &packets[2].data).expect("headers");
    let mut out = Vec::new();
    let mut end = 0;
    for p in &packets[3..] {
        if let Err(e) = d.decode(&p.data, &mut out) {
            eprintln!("packet: {e}");
        }
        if p.granule >= 0 {
            end = p.granule as usize;
        }
    }
    let took = start.elapsed().as_secs_f64();
    out.truncate((end * d.channels()).min(out.len()));
    let pcm: Vec<u8> = out
        .iter()
        .flat_map(|v| ((v * 32768.0).round().clamp(-32768.0, 32767.0) as i16).to_le_bytes())
        .collect();
    std::fs::write(&args[2], pcm).expect("output");
    let secs = out.len() as f64 / d.channels() as f64 / f64::from(d.sample_rate());
    eprintln!(
        "{} channels, {} Hz, {secs:.2} s of sound in {took:.3} s: {:.0}x real time",
        d.channels(),
        d.sample_rate(),
        secs / took
    );
}
