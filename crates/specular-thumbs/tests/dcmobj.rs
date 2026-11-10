//! Thumbnails of DICOM H.264 video (the first frame) and encapsulated PDF (the first page).

use std::sync::Arc;

use specular_core::{CancelToken, Source};
use specular_decode::Registry;
use specular_dicom::fixture;
use specular_thumbs::generate::{GenOptions, generate};

fn thumb(data: Vec<u8>) -> specular_thumbs::pixels::Pixels {
    let s = Registry::default()
        .open(Arc::new(Source::from_vec("x.dcm", data)))
        .unwrap();
    generate(
        &*s,
        128,
        GenOptions::default(),
        &CancelToken::new(),
        &mut |_| {},
    )
    .unwrap()
}

#[test]
fn video_thumbnail_is_the_first_frame() {
    use specular_h264enc as enc;
    let mut e = enc::Encoder::new(enc::Config::new(64, 48, 25, 1, 16)).unwrap();
    let c = vec![128u8; 32 * 24];
    let packets: Vec<_> = [200u8, 30]
        .iter()
        .map(|&l| {
            let y = vec![l; 64 * 48];
            let f = enc::Frame {
                y: &y,
                u: &c,
                v: &c,
                y_stride: 64,
                c_stride: 32,
            };
            e.encode(&f, false)
        })
        .collect();
    let stream = enc::annex_b(&e.sps(), &e.pps(), &packets);
    let file = fixture::video(fixture::H264_HIGH_41, &stream, 0, 64, 48, Some(2), 40.0, 8);
    let px = thumb(file);
    assert_eq!((px.width, px.height), (64, 48));
    // Frame 0 is light (Y 200), not the dark second frame.
    assert!(px.rgba[0] > 200, "{}", px.rgba[0]);
}

#[test]
fn encapsulated_pdf_thumbnail_is_the_first_page() {
    let content = specular_pdf::fixture::fill(200.0, 100.0, [1.0, 0.0, 0.0]);
    let page = specular_pdf::fixture::Page {
        width: 200.0,
        height: 100.0,
        content: &content,
    };
    let pdf = specular_pdf::fixture::build(&[page], "", None, "", &[]);
    let file = fixture::document(fixture::ENCAPSULATED_PDF, "application/pdf", None, &pdf);
    let px = thumb(file);
    assert_eq!(px.width, 128);
    let mid = ((px.height / 2 * px.width + 64) * 4) as usize;
    assert_eq!(&px.rgba[mid..mid + 3], &[255, 0, 0]);
}
