//! Graph building, fusion, ROI propagation and tile planning (no GPU).

use super::testops::*;
use super::tile::{self, Limits, Shape, Tiling};
use super::*;
use crate::model::{ModuleInstance, Value};

fn build(e: &Edit, focus: Option<(ModuleKind, u16)>) -> Graph {
    Graph::build(e, &registry(), focus).expect("graph builds")
}

#[test]
fn point_runs_fuse_and_kernels_split_them() {
    let e = edit(vec![
        gain(0, 2.0),
        add(0, [1.0, 2.0, 3.0]),
        box3(0),
        gain(1, 0.5),
        identity(0),
        add(1, [0.0; 3]),
    ]);
    let g = build(&e, None);
    assert_eq!(
        g.groups,
        vec![
            Group {
                nodes: 0..2,
                fused: true,
                checkpoint: false
            },
            Group {
                nodes: 2..3,
                fused: false,
                checkpoint: true
            },
            Group {
                nodes: 3..6,
                fused: true,
                checkpoint: false
            },
        ]
    );
    // Params: gain (1 word) then add (3 words), in node order.
    assert_eq!(
        g.params(0, 1.0),
        vec![
            2f32.to_bits(),
            1f32.to_bits(),
            2f32.to_bits(),
            3f32.to_bits()
        ]
    );
    assert!(g.params(1, 1.0).is_empty());
}

#[test]
fn focus_splits_fusion_and_checkpoints_before_it() {
    let e = edit(vec![
        gain(0, 2.0),
        add(0, [1.0; 3]),
        gain(1, 3.0),
        add(1, [0.5; 3]),
    ]);
    let g = build(&e, Some((ADD, 1)));
    assert_eq!(g.groups.len(), 2);
    assert_eq!(g.groups[0].nodes, 0..3);
    assert!(g.groups[0].checkpoint);
    assert_eq!(g.groups[1].nodes, 3..4);
    assert!(!g.groups[1].checkpoint);
    // No focus: one fused group, no checkpoint.
    let g = build(&e, None);
    assert_eq!(g.groups.len(), 1);
    assert!(!g.groups[0].checkpoint);
}

#[test]
fn disabled_and_unknown_modules() {
    let mut off = gain(0, 9.0);
    off.enabled = false;
    let unknown = ModuleInstance::new(crate::model::Registry::builtin(), ModuleKind::EXPOSURE, 0);
    let e = edit(vec![off, unknown, add(0, [1.0; 3])]);
    let g = build(&e, None);
    assert_eq!(g.nodes.len(), 1);
    assert_eq!(g.skipped, vec![(ModuleKind::EXPOSURE, 0)]);
    // The node's prefix hash is the edit's own.
    let p = crate::model::hash::prefix_hashes(&e);
    assert_eq!(g.group_hash(0), p[2]);
}

#[test]
fn group_hash_changes_only_downstream() {
    let mut e = edit(vec![gain(0, 2.0), box3(0), gain(1, 3.0)]);
    let a = build(&e, None);
    set(&mut e, GAIN, 1, "gain", Value::Float(4.0));
    let b = build(&e, None);
    assert_eq!(a.group_hash(0), b.group_hash(0));
    assert_eq!(a.group_hash(1), b.group_hash(1));
    assert_ne!(a.group_hash(2), b.group_hash(2));
}

#[test]
fn rois_grow_by_halos_backwards_and_clip() {
    let e = edit(vec![gain(0, 2.0), box3(0), box3(1), add(0, [0.0; 3])]);
    let g = build(&e, None);
    let r = g.rois(Roi::new(10, 10, 20, 20), 100, 100, 1.0);
    assert_eq!(r.len(), 4);
    assert_eq!(r[3].output, Roi::new(10, 10, 20, 20));
    assert_eq!(r[3].input, Roi::new(10, 10, 20, 20));
    assert_eq!(r[2].input, Roi::new(9, 9, 22, 22));
    assert_eq!(r[1].input, Roi::new(8, 8, 24, 24));
    assert_eq!(r[0].input, r[0].output);
    assert_eq!(r[0].output, r[1].input);
    // At the corner the halo clips to the image.
    let r = g.rois(Roi::new(0, 0, 5, 5), 100, 100, 1.0);
    assert_eq!(r[0].input, Roi::new(0, 0, 7, 7));
    // A global node takes its statistics from the whole-image pass: no halo, and it starts
    // a group (the point op after it fuses with it).
    let e = edit(vec![gain(0, 2.0), mean(0), add(0, [0.0; 3])]);
    let g = build(&e, None);
    assert_eq!(g.groups.len(), 2);
    assert_eq!(g.groups[1].nodes, 1..3);
    assert!(g.stats_node(1).is_some() && g.stats_node(0).is_none());
    let r = g.rois(Roi::new(10, 10, 5, 5), 64, 48, 1.0);
    assert_eq!(r[1].input, Roi::new(10, 10, 5, 5));
}

#[test]
fn tiles_cover_the_request_once() {
    let out = Roi::new(3, 5, 1000, 700);
    let tiles = tile::split(out, Some(256));
    let mut seen = vec![0u8; (out.w * out.h) as usize];
    for t in &tiles {
        assert!(out.contains(t));
        for y in t.y..t.bottom() {
            for x in t.x..t.right() {
                seen[((y - out.y) * out.w + (x - out.x)) as usize] += 1;
            }
        }
    }
    assert!(seen.iter().all(|&s| s == 1));
    assert_eq!(tiles.len(), 4 * 3);
}

#[test]
fn plan_tiles_when_over_the_budget_or_a_binding() {
    let e = edit(vec![gain(0, 2.0), box3(0), add(0, [0.0; 3])]);
    let g = build(&e, None);
    let shape = Shape {
        out: Roi::full(6000, 4000),
        width: 6000,
        height: 4000,
        scale: 1.0,
        precision: Precision::F32,
        source_bpp: 16,
    };
    let roomy = Limits {
        max_binding: 1 << 31,
        offset_align: 256,
        working: 4 << 30,
    };
    let p = tile::plan(&g, &shape, Tiling::Auto, &roomy).expect("fits");
    assert_eq!(p.side, None);
    assert_eq!(p.tiles.len(), 1);
    // 24 MP f32 intermediates are 384 MB: a 400 MB working budget tiles.
    let tight = Limits {
        working: 400 << 20,
        ..roomy
    };
    let p = tile::plan(&g, &shape, Tiling::Auto, &tight).expect("fits tiled");
    assert_eq!(p.side, Some(2048));
    assert!(p.peak <= tight.working);
    // A 128 MiB binding limit tiles even with room to spare.
    let small = Limits {
        max_binding: 128 << 20,
        ..roomy
    };
    let p = tile::plan(&g, &shape, Tiling::Auto, &small).expect("fits tiled");
    assert!(p.side.is_some());
    // Off refuses, with what it would need.
    let e = tile::plan(&g, &shape, Tiling::Off, &tight).unwrap_err();
    assert!(e.needs > tight.working);
}

#[test]
fn params_too_large_is_refused() {
    struct Big;
    impl Op for Big {
        fn name(&self) -> &'static str {
            "big"
        }
        fn kind(&self) -> NodeKind {
            NodeKind::Point
        }
        fn param_words(&self) -> u32 {
            PARAM_WORDS
        }
        fn write_params(&self, _: &Params, _: f32, _: &mut [u32]) {}
        fn wgsl(&self) -> &'static str {
            "return c;"
        }
    }
    let mut r = registry();
    r.register(GAIN, 1, Arc::new(Big));
    let e = edit(vec![gain(0, 1.0)]);
    assert_eq!(
        Graph::build(&e, &r, None).unwrap_err(),
        GraphError::ParamsTooLarge("big")
    );
}

#[test]
fn fused_groups_split_when_params_overflow() {
    // 492 param words per dispatch: 200 adds of 3 words need two groups.
    let e = edit((0..200).map(|i| add(i, [0.0; 3])).collect());
    let g = build(&e, None);
    assert_eq!(g.groups.len(), 2);
    assert_eq!(g.groups[0].nodes, 0..164);
}
