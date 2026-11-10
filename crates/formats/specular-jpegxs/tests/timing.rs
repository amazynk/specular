//! Decode timing of one codestream (`tools/corpus/fetch-jpegxs.sh --4k` makes a 4096×2160
//! 4:2:2 10-bit frame):
//!
//! `SPECULAR_JPEGXS_TIMING=tools/corpus/jpegxs/synth/4k-main422-10.jxs cargo test -p specular-jpegxs --test timing -- --nocapture`

use std::time::Instant;

#[test]
fn time_decode() {
    let Some(path) = std::env::var_os("SPECULAR_JPEGXS_TIMING") else {
        eprintln!("SPECULAR_JPEGXS_TIMING not set");
        return;
    };
    let data = std::fs::read(&path).expect("codestream");
    let runs = 7;
    let mut samples = f64::MAX;
    let mut pixels = f64::MAX;
    for _ in 0..runs {
        let t = Instant::now();
        let s = specular_jpegxs::decode_samples(&data).expect("decodes");
        samples = samples.min(t.elapsed().as_secs_f64() * 1000.0);
        assert!(!s.truncated);
        let t = Instant::now();
        let i = specular_jpegxs::decode(&data, 0).expect("decodes");
        pixels = pixels.min(t.elapsed().as_secs_f64() * 1000.0);
        assert!(i.width > 0);
    }
    let i = specular_jpegxs::info(&data, 0).unwrap();
    eprintln!(
        "{}: {}×{} {}; samples {samples:.1} ms, display pixels {pixels:.1} ms (best of {runs}, {} threads)",
        std::path::Path::new(&path).display(),
        i.width,
        i.height,
        i.coding,
        rayon::current_num_threads()
    );
}
