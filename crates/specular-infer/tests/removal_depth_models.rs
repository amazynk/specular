//! The object removal and depth networks end to end, GPU against the CPU reference, with fixed-seed
//! random weights at the real widths on small inputs: Big-LaMa (FFC blocks with their Fourier
//! units) and Depth Anything V2 Small (ViT-S/14 and the DPT head). The real weights are checked the
//! same way in the oracle run (`tests/removal_depth_oracle.rs`).

mod common;

use common::check;
use specular_infer::depth::{self, DepthConfig};
use specular_infer::lama;
use specular_infer::models::RandomWeights;
use specular_infer::tensor::Rng;
use specular_infer::{Feed, Graph};

#[test]
fn lama_gpu_matches_cpu() {
    let (w, h) = (40, 32);
    let g = lama::graph(&mut RandomWeights::new(27), (w, h)).unwrap();
    assert_eq!(g.shape(g.outputs()[0]), [1, 3, h, w]);
    let mut rng = Rng::new(270);
    let rgb: Vec<f32> = rng
        .tensor(&[3 * w * h], 0.5)
        .to_f32()
        .iter()
        .map(|v| v + 0.5)
        .collect();
    let mask: Vec<f32> = (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as f32, (i / w) as f32);
            if (x - 20.0).powi(2) + (y - 15.0).powi(2) < 64.0 {
                1.0
            } else {
                0.0
            }
        })
        .collect();
    let x = lama::input(&rgb, &mask, (w, h));
    // 50 convs deep with sums of up to 4,608 terms, sigmoid out: f32 order differences.
    let (out, d) = check("lama", &g, &[Feed::Tensor(&x)], 1e-4, 1e-3);
    let v = out[0].to_f32();
    assert!(v.iter().all(|a| (0.0..=1.0).contains(a)));
    eprintln!("lama 40x32: worst |gpu − cpu| {:.2e}", d.max_abs);
}

#[test]
fn lama_refuses_sides_off_the_step() {
    assert!(lama::graph(&mut RandomWeights::new(1), (36, 32)).is_err());
    assert!(lama::graph(&mut RandomWeights::new(1), (8, 8)).is_err());
}

#[test]
fn depth_gpu_matches_cpu() {
    let c = DepthConfig::small();
    let (w, h) = (4 * depth::PATCH, 3 * depth::PATCH);
    let mut g = Graph::new();
    let x = g.input(&[1, 3, h, w], specular_infer::DType::F32).unwrap();
    let y = depth::depth(&mut g, x, &mut RandomWeights::new(28), &c).unwrap();
    g.output(y).unwrap();
    assert_eq!(g.shape(y), [1, 1, h, w]);
    let mut rng = Rng::new(280);
    let input = rng.tensor(&[1, 3, h, w], 1.0);
    let (_, d) = check("depth", &g, &[Feed::Tensor(&input)], 1e-4, 1e-3);
    eprintln!("depth 56x42: worst |gpu − cpu| {:.2e}", d.max_abs);
}

#[test]
fn depth_input_follows_the_reference_rule() {
    // The reference processor: the scale that changes the picture least, sides rounded to
    // multiples of 14.
    assert_eq!(depth::input_size(1024, 768, 518), (686, 518));
    assert_eq!(depth::input_size(768, 1024, 518), (518, 686));
    assert_eq!(depth::input_size(518, 518, 518), (518, 518));
    assert_eq!(depth::input_size(300, 200, 518), (518, 350));
}

#[test]
fn position_table_keeps_its_own_grid() {
    let g = 3;
    let d = 2;
    let table: Vec<f32> = (0..(1 + g * g) * d).map(|v| v as f32).collect();
    let same = depth::resize_positions(&table, g, d, (g, g));
    assert_eq!(same.to_f32(), table);
    // A constant table stays constant at any size (the cubic weights sum to 1).
    let flat = vec![0.25f32; (1 + g * g) * d];
    let big = depth::resize_positions(&flat, g, d, (5, 7));
    assert_eq!(big.shape, [1, 1 + 35, d]);
    assert!(big.to_f32().iter().all(|v| (v - 0.25).abs() < 1e-6));
}
