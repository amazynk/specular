//! Hostile `.safetensors` files: the parser must return an error or tensors whose sizes
//! match their shapes and lie inside the file, never panic or allocate beyond the input.
//! Whatever parses is also offered to the model builders as weights (shape checks).

#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_infer::models::{cnn, image_batch, vit, CnnConfig, VitConfig};
use specular_infer::safetensors::SafeTensors;
use specular_infer::tensor::numel;
use specular_infer::Graph;

fuzz_target!(|data: &[u8]| {
    let Ok(mut st) = SafeTensors::parse(data) else {
        return;
    };
    assert!(st.header.data_start <= data.len() as u64);
    let infos = st.header.tensors.clone();
    for info in &infos {
        assert!(info.begin <= info.end && st.header.data_start + info.end <= data.len() as u64);
        let bytes = st.bytes(&info.name).expect("listed tensors have bytes");
        assert_eq!(bytes.len() as u64, info.end - info.begin);
        if let Ok(t) = st.tensor(&info.name) {
            assert_eq!(Some(t.len()), numel(&info.shape));
            assert_eq!(t.shape, info.shape);
        }
    }
    let mut g = Graph::new();
    if let Ok(x) = image_batch(&mut g, 1, 32) {
        let _ = vit(&mut g, x, &VitConfig::test(), &mut st);
    }
    let mut g = Graph::new();
    if let Ok(x) = image_batch(&mut g, 1, 64) {
        let _ = cnn(&mut g, x, &CnnConfig::test(), &mut st);
    }
});
