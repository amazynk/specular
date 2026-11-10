//! Writes an AC-3 stream with its dither flags cleared (`specular_ac3::without_dither`), so
//! another decoder's output has no random noise in it: `cargo run --example ac3_undither --
//! IN OUT`. Prints how many frames dithered.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&args[1]).expect("input");
    let out = specular_ac3::without_dither(&data);
    let changed = data.iter().zip(&out).filter(|(a, b)| a != b).count();
    std::fs::write(&args[2], &out).expect("output");
    eprintln!("{} bytes, {changed} changed", out.len());
}
