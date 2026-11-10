//! Headers asking for far more memory than the file could fill.

mod common;

use common::*;
use specular_psd::*;

/// A 20000×20000 RGB header with an RLE row table of zero-length rows and nothing else
/// (120 KB): the 1.2 GB composite was allocated (and filled) before any pixel data.
#[test]
fn rle_row_table_alone_is_refused() {
    let (w, h) = (20_000u32, 20_000u32);
    let mut f = b"8BPS\0\x01\0\0\0\0\0\0\0\x03".to_vec();
    f.extend_from_slice(&h.to_be_bytes());
    f.extend_from_slice(&w.to_be_bytes());
    f.extend_from_slice(&8u16.to_be_bytes());
    f.extend_from_slice(&3u16.to_be_bytes());
    f.extend_from_slice(&[0; 12]);
    f.extend_from_slice(&1u16.to_be_bytes());
    f.resize(f.len() + 3 * h as usize * 2, 0);
    let p = parse(&f).unwrap();
    assert!(matches!(
        p.decode_planes(&f, &Options::default()),
        Err(Error::Truncated(_))
    ));
}

/// A small document whose header is patched to a 20000×20000 canvas: its one 8×8 layer was
/// decoded onto a 1.6 GB canvas.
#[test]
fn layer_on_a_huge_canvas_is_refused() {
    let bounds = [0, 0, 8, 8];
    let rows = vec![200u8; 64];
    let layer = LayerSpec {
        name: "l".into(),
        bounds,
        opacity: 255,
        compression: 0,
        channels: vec![
            (-1, rows.clone()),
            (0, rows.clone()),
            (1, rows.clone()),
            (2, rows),
        ],
        ..Default::default()
    };
    let planes = (0..3).map(|c| pattern(16, 16, c + 1, 1)).collect();
    let mut s = Spec::new(16, 16, 8, 3, planes);
    s.layer_count = 1;
    s.layers = vec![layer];
    let mut f = write(&s);
    let ok = parse(&f).unwrap();
    assert!(
        ok.decode_layer_planes(&f, 0, &Options::default(), None)
            .is_ok()
    );
    f[14..18].copy_from_slice(&20_000u32.to_be_bytes());
    f[18..22].copy_from_slice(&20_000u32.to_be_bytes());
    let p = parse(&f).unwrap();
    assert!(matches!(
        p.decode_layer_planes(&f, 0, &Options::default(), None),
        Err(Error::Truncated(_))
    ));
}
