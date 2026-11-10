//! Lists an MPEG-1/2 elementary stream's sequence header and pictures:
//! `cargo run --example m2v_info -- IN.m2v`.

fn main() {
    let path = std::env::args().nth(1).expect("input");
    let data = std::fs::read(path).expect("input");
    let s = specular_mpeg2::scan(&data);
    println!("{:?}", s.sequence);
    for (i, p) in s.pictures.iter().enumerate() {
        println!("{i}: {p:?}");
    }
}
