//! Hostile nesting: rendering must not recurse once per frame.

use specular_jxl::JxlFile;

/// 3000 layers, each added onto the one before (`gen_layers.py`). Blending a frame blends
/// the frame it is drawn onto first, so this recursed 3000 levels deep and overflowed the
/// stack; now references are blended oldest first.
#[test]
fn long_layer_chain_renders_on_a_small_stack() {
    let data = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/layers_chain.jxl"
    ))
    .unwrap();
    let first = specular_thread::on_stack(256 * 1024, || {
        let f = JxlFile::open(&data[..]).unwrap();
        let d = f.decode(1, false).unwrap();
        d.data[0]
    });
    // Pixel (0, 0) got all 3000 additions; whether they wrap or clamp, it is not black.
    assert_ne!(first, 0);
}
