//! Prints an XCF file's header, layer list and a few composited pixels (a debugging aid).
//! Usage: cargo run --example xcf-dump -- FILE [X,Y...]; `XCF_DUMP_RAW=PATH` also writes the
//! composited pixels there.
use specular_xcf::{Options, decompress, parse};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: xcf-dump FILE [X,Y...]");
    let raw = std::fs::read(&path).expect("read");
    let data = decompress(&raw).expect("container").unwrap_or(raw);
    let x = parse(&data).expect("parse");
    println!("{:?}", x.info());
    for l in x.layers() {
        println!("  {:?}", l);
    }
    let img = x.render(&data, &Options::default()).expect("render");
    println!(
        "{:?} {}x{} truncated {}",
        img.format, img.width, img.height, img.truncated
    );
    if let Some(out) = std::env::var_os("XCF_DUMP_RAW") {
        std::fs::write(out, &img.data).expect("write raw");
    }
    let bpp = img.format.bytes_per_pixel();
    for a in args {
        let (px, py) = a.split_once(',').expect("X,Y");
        let (px, py): (usize, usize) = (px.parse().unwrap(), py.parse().unwrap());
        let o = (py * img.width as usize + px) * bpp;
        println!("  {px},{py}: {:?}", &img.data[o..o + bpp]);
    }
}
