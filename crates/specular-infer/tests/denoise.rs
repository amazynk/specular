//! AI denoise: the ops NAFNet added (simple gate, depth to space, long-row
//! reductions), GPU against the CPU reference, and NAFNet whole with fixed-seed random
//! weights (f32 and f16). The real checkpoint is checked against its reference in
//! `denoise_oracle.rs`.

mod common;

use common::check;
use specular_infer::ffdnet::{self, FfdConfig};
use specular_infer::graph::ReduceOp;
use specular_infer::models::RandomWeights;
use specular_infer::nafnet::{self, NafConfig};
use specular_infer::tensor::Rng;
use specular_infer::tiles::{Cells, Planar, Rows, Tiling};
use specular_infer::{DType, Feed, Graph, Tensor};

/// Whole networks: f32 sums over thousands of terms in a different order.
const NET: (f32, f32) = (2e-3, 2e-3);

#[test]
fn gate() {
    let mut rng = Rng::new(31);
    for shape in [[1, 2, 1, 1], [2, 6, 5, 3], [1, 64, 17, 9]] {
        let x = rng.tensor(&shape, 2.0);
        let mut g = Graph::new();
        let xi = g.input(&shape, DType::F32).unwrap();
        let y = g.gate(xi).unwrap();
        g.output(y).unwrap();
        let (out, _) = check(
            &format!("gate {shape:?}"),
            &g,
            &[Feed::Tensor(&x)],
            0.0,
            0.0,
        );
        let (c, hw) = (shape[1] / 2, shape[2] * shape[3]);
        assert_eq!(out[0].shape, [shape[0], c, shape[2], shape[3]]);
        let xv = x.to_f32();
        let ov = out[0].to_f32();
        for n in 0..shape[0] {
            for k in 0..c * hw {
                let a = xv[n * 2 * c * hw + k];
                let b = xv[n * 2 * c * hw + c * hw + k];
                assert_eq!(ov[n * c * hw + k], a * b);
            }
        }
    }
    let mut g = Graph::new();
    let xi = g.input(&[1, 3, 2, 2], DType::F32).unwrap();
    assert!(g.gate(xi).is_err());
}

#[test]
fn depth_to_space() {
    let mut rng = Rng::new(37);
    for (shape, r) in [([1, 4, 1, 1], 2), ([2, 12, 3, 5], 2), ([1, 18, 4, 3], 3)] {
        let x = rng.tensor(&shape, 1.0);
        let mut g = Graph::new();
        let xi = g.input(&shape, DType::F32).unwrap();
        let y = g.depth_to_space(xi, r).unwrap();
        g.output(y).unwrap();
        let name = format!("depth to space {shape:?} by {r}");
        let (out, _) = check(&name, &g, &[Feed::Tensor(&x)], 0.0, 0.0);
        let (n, c, h, w) = (shape[0], shape[1] / (r * r), shape[2], shape[3]);
        assert_eq!(out[0].shape, [n, c, h * r, w * r]);
        // PyTorch's PixelShuffle: out[n, c, y·r + i, x·r + j] = in[n, c·r² + i·r + j, y, x].
        let (xv, ov) = (x.to_f32(), out[0].to_f32());
        for b in 0..n {
            for ch in 0..c {
                for y in 0..h * r {
                    for xx in 0..w * r {
                        let ic = ch * r * r + (y % r) * r + xx % r;
                        let src = ((b * shape[1] + ic) * h + y / r) * w + xx / r;
                        let dst = ((b * c + ch) * h * r + y) * w * r + xx;
                        assert_eq!(ov[dst], xv[src], "{name} at {b} {ch} {y} {xx}");
                    }
                }
            }
        }
    }
    let mut g = Graph::new();
    let xi = g.input(&[1, 6, 2, 2], DType::F32).unwrap();
    assert!(g.depth_to_space(xi, 2).is_err());
}

/// Sums, means and maxima over long contiguous rows (global pooling of large maps) take
/// the workgroup-per-row path; short rows and strided axes the thread-per-output path.
#[test]
fn long_row_reductions() {
    let mut rng = Rng::new(41);
    for (shape, axis) in [
        (vec![1, 32, 256 * 256], 2),
        (vec![3, 1500], 1),
        (vec![2, 1023], 1),
        (vec![2, 4096, 3], 1),
    ] {
        let x = rng.tensor(&shape, 1.0);
        for op in [ReduceOp::Sum, ReduceOp::Mean, ReduceOp::Max] {
            let mut g = Graph::new();
            let xi = g.input(&shape, DType::F32).unwrap();
            let y = g.reduce(xi, op, axis).unwrap();
            g.output(y).unwrap();
            // Sums of up to 65,536 terms in a different order.
            let atol = if op == ReduceOp::Sum { 2e-3 } else { 1e-5 };
            check(
                &format!("reduce {op:?} {shape:?} axis {axis}"),
                &g,
                &[Feed::Tensor(&x)],
                atol,
                1e-4,
            );
        }
    }
    // A global average pool of a 256² map, as NAFNet's channel attention takes it.
    let x = rng.tensor(&[1, 16, 256, 256], 1.0);
    let mut g = Graph::new();
    let xi = g.input(&[1, 16, 256, 256], DType::F32).unwrap();
    let m = g.global_avg_pool(xi).unwrap();
    g.output(m).unwrap();
    let (out, _) = check("global avg pool 256²", &g, &[Feed::Tensor(&x)], 1e-6, 1e-4);
    let xv = x.to_f32();
    for (c, &v) in out[0].to_f32().iter().enumerate() {
        let want = xv[c * 65536..(c + 1) * 65536]
            .iter()
            .map(|&a| f64::from(a))
            .sum::<f64>()
            / 65536.0;
        assert!(
            (f64::from(v) - want).abs() < 1e-5,
            "channel {c}: {v} vs {want}"
        );
    }
}

fn small() -> NafConfig {
    NafConfig {
        width: 8,
        enc: vec![1, 1, 1, 2],
        middle: 2,
        dec: vec![1, 1, 1, 1],
    }
}

#[test]
fn nafnet_gpu_matches_cpu() {
    for f16 in [false, true] {
        let mut w = RandomWeights { seed: 11, f16 };
        let c = small();
        let g = nafnet::graph(&mut w, &c, (48, 32)).unwrap();
        let x = Rng::new(3).tensor(&[1, 3, 32, 48], 0.5);
        let (out, d) = check(
            &format!("nafnet small f16={f16}"),
            &g,
            &[Feed::Tensor(&x)],
            NET.0,
            NET.1,
        );
        assert_eq!(out[0].shape, [1, 3, 32, 48]);
        assert!(d.cosine > 0.99999, "{d:?}");
        assert!(out[0].to_f32().iter().all(|v| v.is_finite()));
    }
    // The SIDD width-32 layout, as shipped (one 64² tile, f16 weights).
    let mut w = RandomWeights {
        seed: 12,
        f16: true,
    };
    let g = nafnet::graph(&mut w, &NafConfig::sidd(32), (64, 64)).unwrap();
    let x = Rng::new(4).tensor(&[1, 3, 64, 64], 0.5);
    let (_, d) = check("nafnet sidd-32 64²", &g, &[Feed::Tensor(&x)], NET.0, NET.1);
    assert!(d.cosine > 0.99999, "{d:?}");
}

#[test]
fn nafnet_shapes() {
    let mut w = RandomWeights::new(1);
    assert!(nafnet::graph(&mut w, &small(), (40, 32)).is_err());
    assert!(nafnet::graph(&mut w, &small(), (32, 0)).is_err());
    assert_eq!(small().side_step(), 16);
}

// -- FFDNet ------------------------------------------------------------------------------

fn small_ffd() -> FfdConfig {
    FfdConfig {
        channels: 3,
        width: 16,
        layers: 4,
    }
}

#[test]
fn space_to_depth_inverts_depth_to_space() {
    let shape = [1, 4, 6, 10];
    let x = Rng::new(43).tensor(&shape, 1.0);
    let mut g = Graph::new();
    let xi = g.input(&shape, DType::F32).unwrap();
    let s = ffdnet::space_to_depth2(&mut g, xi).unwrap();
    assert_eq!(g.shape(s), [1, 16, 3, 5]);
    let back = g.depth_to_space(s, 2).unwrap();
    g.output(s).unwrap();
    g.output(back).unwrap();
    let (out, _) = check("space to depth", &g, &[Feed::Tensor(&x)], 0.0, 0.0);
    assert_eq!(out[1].to_f32(), x.to_f32());
    // PixelUnshuffle: channel c·4 + i·2 + j at (y, x) is (2y + i, 2x + j) of channel c.
    let (xv, sv) = (x.to_f32(), out[0].to_f32());
    assert_eq!(sv[((2 * 4 + 3) * 3 + 1) * 5 + 2], xv[(2 * 6 + 3) * 10 + 5]);
}

#[test]
fn ffdnet_gpu_matches_cpu() {
    for f16 in [false, true] {
        let mut w = RandomWeights { seed: 13, f16 };
        let g = ffdnet::graph(&mut w, &small_ffd(), (36, 20)).unwrap();
        let x = Rng::new(5).tensor(&[1, 3, 20, 36], 0.5);
        let sigma = Tensor::f32(&[1, 1, 1, 1], vec![0.05]);
        let (out, d) = check(
            &format!("ffdnet small f16={f16}"),
            &g,
            &[Feed::Tensor(&x), Feed::Tensor(&sigma)],
            NET.0,
            NET.1,
        );
        assert_eq!(out[0].shape, [1, 3, 20, 36]);
        assert!(d.cosine > 0.99999, "{d:?}");
    }
    // The colour model's layout, f16 weights, one 64² tile.
    let mut w = RandomWeights {
        seed: 14,
        f16: true,
    };
    let g = ffdnet::graph(&mut w, &FfdConfig::color(), (64, 64)).unwrap();
    let x = Rng::new(6).tensor(&[1, 3, 64, 64], 0.5);
    let sigma = Tensor::f32(&[1, 1, 1, 1], vec![0.02]);
    let (_, d) = check(
        "ffdnet colour 64²",
        &g,
        &[Feed::Tensor(&x), Feed::Tensor(&sigma)],
        NET.0,
        NET.1,
    );
    assert!(d.cosine > 0.99999, "{d:?}");
    let mut w = RandomWeights::new(1);
    assert!(ffdnet::graph(&mut w, &small_ffd(), (35, 20)).is_err());
}

/// FFDNet looks no further than its reach, so tiles with a larger margin join exactly:
/// the tiled result equals the whole picture's.
#[test]
fn ffdnet_tiles_equal_the_whole_picture() {
    let (w, h) = (150, 100);
    let img = ramp_image(w, h);
    let mut wts = RandomWeights::new(15);
    let c = small_ffd();
    let tiles = Tiling {
        size: 64,
        margin: 16,
        overlap: 8,
    };
    let tg = ffdnet::graph(&mut wts, &c, (tiles.size, tiles.size)).unwrap();
    let whole = ffdnet::graph(&mut wts, &c, (w, h)).unwrap();
    let sigma = Tensor::f32(&[1, 1, 1, 1], vec![0.03]);
    let x = Tensor::f32(&[1, 3, h, w], img.clone());
    let want = specular_infer::cpu::run(&whole, &[Feed::Tensor(&x), Feed::Tensor(&sigma)], 4)
        .unwrap()[0]
        .to_f32();
    let all = tiles.cells_of((w, h), 0, 0, w, h);
    let out = tiled(tiles, &img, (w, h), all, &mut |t| {
        Ok(specular_infer::cpu::run(&tg, &[Feed::Tensor(t), Feed::Tensor(&sigma)], 4)?.remove(0))
    });
    // The small net reaches 4 half-resolution pixels (9 full ones): under the margin. At
    // the picture's own edges tiles read the edge pixels repeated where the whole picture
    // reads zeros: left out.
    let mut worst = 0f32;
    for c in 0..3 {
        for y in 10..h - 10 {
            for x in 10..w - 10 {
                let i = (c * h + y) * w + x;
                worst = worst.max((out[i] - want[i]).abs());
            }
        }
    }
    assert!(worst < 1e-5, "tiled vs whole: {worst}");
}

// -- Tiling ------------------------------------------------------------------------------

const SMALL_TILES: Tiling = Tiling {
    size: 64,
    margin: 8,
    overlap: 16,
};

fn ramp_image(w: usize, h: usize) -> Vec<f32> {
    let mut v = vec![0f32; 3 * w * h];
    for c in 0..3 {
        for y in 0..h {
            for x in 0..w {
                v[(c * h + y) * w + x] =
                    0.3 + 0.2 * ((x as f32 * 0.07 + c as f32).sin() * (y as f32 * 0.05).cos());
            }
        }
    }
    v
}

type Net<'a> = &'a mut dyn FnMut(&Tensor) -> Result<Tensor, specular_infer::Error>;

/// Runs `net` tiled over `cells` of a `w × h` planar image; the output over the cells'
/// pixels (zeros elsewhere).
fn tiled(t: Tiling, img: &[f32], (w, h): (usize, usize), cells: Cells, net: Net<'_>) -> Vec<f32> {
    let mut out = vec![0f32; 3 * w * h];
    let src = Planar { w, h, data: img };
    let mut emit = |r: Rows<'_>| {
        for c in 0..3 {
            for y in 0..r.h {
                for x in 0..r.w {
                    out[(c * h + r.y + y) * w + r.x + x] = r.data[(c * r.h + y) * r.w + x];
                }
            }
        }
    };
    assert!(
        t.run(&src, cells, net, &mut emit, &mut |_, _| true)
            .unwrap()
    );
    out
}

/// A tile-dependent "network": the input plus a share of the tile's own mean (as NAFNet's
/// channel attention depends on its tile).
fn per_tile(t: &Tensor) -> Result<Tensor, specular_infer::Error> {
    let v = t.to_f32();
    let mean = v.iter().sum::<f32>() / v.len() as f32;
    Ok(Tensor::f32(
        &t.shape,
        v.iter().map(|&a| a + 0.5 * mean).collect(),
    ))
}

#[test]
fn tiles_rejoin_an_identity_network_exactly() {
    let (w, h) = (150, 97);
    let img = ramp_image(w, h);
    let all = SMALL_TILES.cells_of((w, h), 0, 0, w, h);
    let out = tiled(SMALL_TILES, &img, (w, h), all, &mut |t| Ok(t.clone()));
    let worst = img
        .iter()
        .zip(&out)
        .map(|(a, b)| (a - b).abs())
        .fold(0f32, f32::max);
    assert!(worst < 1e-6, "identity through tiles: {worst}");
    assert_eq!(SMALL_TILES.stride(), 32);
    assert_eq!(SMALL_TILES.cells(w, h), (5, 4));
}

#[test]
fn regions_agree_with_the_whole_image() {
    let (w, h) = (150, 97);
    let img = ramp_image(w, h);
    let all = SMALL_TILES.cells_of((w, h), 0, 0, w, h);
    let whole = tiled(SMALL_TILES, &img, (w, h), all, &mut per_tile);
    // Regions computed apart (as separate 100 % views would be).
    for cells in [
        Cells {
            x0: 0,
            y0: 0,
            x1: 2,
            y1: 4,
        },
        Cells {
            x0: 2,
            y0: 1,
            x1: 5,
            y1: 3,
        },
        Cells {
            x0: 4,
            y0: 3,
            x1: 5,
            y1: 4,
        },
    ] {
        let part = tiled(SMALL_TILES, &img, (w, h), cells, &mut per_tile);
        let s = SMALL_TILES.stride();
        for c in 0..3 {
            for y in cells.y0 * s..(cells.y1 * s).min(h) {
                for x in cells.x0 * s..(cells.x1 * s).min(w) {
                    let i = (c * h + y) * w + x;
                    assert_eq!(part[i], whole[i], "{cells:?} at {x},{y}");
                }
            }
        }
    }
}

/// The largest change between neighbouring pixels that the tiling added (outside `skip`).
fn worst_step(
    out: &[f32],
    img: &[f32],
    w: usize,
    h: usize,
    skip: (usize, usize, usize, usize),
) -> f32 {
    let fd = |v: &[f32], x: usize, y: usize| v[y * w + x + 1] - v[y * w + x];
    let mut worst = 0f32;
    for y in 0..h {
        for x in 0..w - 1 {
            if (skip.0..=skip.2).contains(&x) && (skip.1..=skip.3).contains(&y) {
                continue;
            }
            worst = worst.max((fd(out, x, y) - fd(img, x, y)).abs());
        }
    }
    worst
}

/// Seams: a network whose output shifts with each tile's content leaves steps at hard tile
/// joins; cross-faded over the overlap they spread out to a ramp.
#[test]
fn tile_borders_leave_no_steps() {
    let (w, h) = (200, 140);
    // A flat image with a bright block: tiles see different means, so their shifts differ.
    let mut img = vec![0.2f32; 3 * w * h];
    for c in 0..3 {
        for y in 40..90 {
            for x in 60..130 {
                img[(c * h + y) * w + x] = 0.9;
            }
        }
    }
    let skip = (58, 38, 131, 91);
    let all = SMALL_TILES.cells_of((w, h), 0, 0, w, h);
    let out = tiled(SMALL_TILES, &img, (w, h), all, &mut per_tile);
    let faded = worst_step(&out, &img, w, h, skip);
    let hard_tiles = Tiling {
        overlap: 0,
        margin: 8,
        size: 64,
    };
    let all = hard_tiles.cells_of((w, h), 0, 0, w, h);
    let hard_out = tiled(hard_tiles, &img, (w, h), all, &mut per_tile);
    let hard = worst_step(&hard_out, &img, w, h, skip);
    eprintln!("largest step: cross-faded {faded:.2e}, hard joins {hard:.2e}");
    assert!(hard > 0.02, "the test network must shift per tile: {hard}");
    assert!(faded < hard / 8.0, "cross-faded {faded} vs hard {hard}");
}

#[test]
fn a_run_stops_when_asked() {
    let (w, h) = (150, 97);
    let img = ramp_image(w, h);
    let src = Planar { w, h, data: &img };
    let all = SMALL_TILES.cells_of((w, h), 0, 0, w, h);
    let total = SMALL_TILES.tiles_for((w, h), all);
    assert_eq!(total, 20);
    let mut runs = 0;
    let finished = SMALL_TILES
        .run(
            &src,
            all,
            &mut |t| {
                runs += 1;
                Ok(t.clone())
            },
            &mut |_| {},
            &mut |done, n| {
                assert_eq!(n, total);
                done < 3
            },
        )
        .unwrap();
    assert!(!finished);
    assert_eq!(runs, 3);
}
