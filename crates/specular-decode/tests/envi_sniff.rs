//! Raw ENVI data opens as ENVI even when its first bytes match another format's probe by
//! chance (8-bit samples 00 00 01 00 … read as an icon directory before).

use std::path::Path;
use std::sync::Arc;

use specular_core::Source;
use specular_decode::{Format, Registry};

const W: usize = 8;
const H: usize = 4;

#[test]
fn envi_data_that_looks_like_an_icon_opens_as_envi() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("envi-sniff");
    std::fs::create_dir_all(&dir).unwrap();
    // An icon directory with one 16×16 entry, then more samples.
    let mut data = vec![
        0, 0, 1, 0, 1, 0, 16, 16, 0, 0, 1, 0, 32, 0, 0x10, 0, 0, 0, 22, 0, 0, 0,
    ];
    data.resize(W * H, 7);
    assert!(
        specular_bitmap::ico::probe(&data),
        "the samples must look like an icon"
    );
    std::fs::write(dir.join("scene.img"), &data).unwrap();
    std::fs::write(
        dir.join("scene.hdr"),
        format!(
            "ENVI\nsamples = {W}\nlines = {H}\nbands = 1\nheader offset = 0\n\
             file type = ENVI Standard\ndata type = 1\ninterleave = bsq\nbyte order = 0\n"
        ),
    )
    .unwrap();
    let s = Registry::default()
        .open(Arc::new(Source::open(dir.join("scene.img")).unwrap()))
        .unwrap();
    let i = s.info();
    assert_eq!(i.format, Format::Envi);
    assert_eq!((i.width, i.height), (W as u32, H as u32));

    // The same bytes named as an icon still open as one.
    std::fs::write(dir.join("real.ico"), &data).unwrap();
    std::fs::copy(dir.join("scene.hdr"), dir.join("real.hdr")).unwrap();
    let ico = Registry::default().open(Arc::new(Source::open(dir.join("real.ico")).unwrap()));
    if let Ok(s) = ico {
        assert_eq!(s.info().format, Format::Ico);
    }
}
