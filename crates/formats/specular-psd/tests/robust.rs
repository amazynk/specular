//! Truncation and mutation loops: parse and decode must never panic.
mod common;

use common::*;
use specular_psd::*;

fn corpus() -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for (depth, mode, n, comp, psb) in [
        (8u16, 3u16, 3u32, 1u16, false),
        (8, 3, 4, 0, false),
        (8, 3, 3, 2, false),
        (8, 1, 1, 3, false),
        (16, 3, 3, 1, true),
        (16, 4, 4, 3, false),
        (32, 3, 3, 3, false),
        (32, 1, 1, 1, true),
        (1, 0, 1, 1, false),
        (8, 9, 3, 1, false),
        (8, 2, 1, 1, false),
    ] {
        let (w, h) = (23u32, 7u32);
        let bps = (depth as usize / 8).max(1);
        let planes = (0..n)
            .map(|c| {
                if depth == 1 {
                    pattern(3, h, c, 1)
                } else {
                    pattern(w, h, c, bps)
                }
            })
            .collect();
        let mut s = Spec::new(w, h, depth, mode, planes);
        s.compression = comp;
        s.psb = psb;
        if mode == 2 {
            s.color_data = vec![7; 768];
        }
        s.resources
            .push((1057, version_info(true, "Adobe Photoshop")));
        s.resources.push((1039, vec![1, 2, 3]));
        s.layer_count = -1;
        s.layers = vec![LayerSpec {
            name: "x".into(),
            bounds: [0, 0, 7, 23],
            opacity: 200,
            hidden: false,
            section: 0,
            ..Default::default()
        }];
        s.lr16 = depth == 16;
        out.push(write(&s));
    }
    out
}

fn exercise(f: &[u8]) {
    if let Ok(p) = parse(f) {
        let _ = p.thumbnail(f);
        let _ = p.icc(f);
        if let Ok(pl) = p.decode_planes(f, &Options::default()) {
            let _ = pl.to_rgba8();
        }
        if let Ok(img) = p.decode_rgba8(f, &Options { parallel: false }) {
            assert_eq!(
                img.data.len(),
                img.width as usize * img.height as usize * img.format.channels()
            );
        }
    }
}

#[test]
fn every_truncation() {
    for f in corpus() {
        exercise(&f);
        for n in 0..f.len() {
            exercise(&f[..n]);
        }
    }
}

#[test]
fn truncated_pixels_are_flagged() {
    for f in corpus() {
        let p = parse(&f).unwrap();
        let img = p.decode_rgba8(&f, &Options::default()).unwrap();
        assert!(!img.truncated);
        let cut = &f[..f.len() - (f.len() / 6).max(5)];
        if let Ok(img) = parse(cut).unwrap().decode_rgba8(cut, &Options::default()) {
            assert!(img.truncated, "{:?}", p.info());
        }
    }
}

#[test]
fn random_mutations() {
    let mut s = 0x1234_5678u32;
    let mut rnd = || {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        s
    };
    for f in corpus() {
        for _ in 0..1500 {
            let mut g = f.clone();
            for _ in 0..1 + rnd() % 4 {
                let i = rnd() as usize % g.len();
                g[i] = match rnd() % 4 {
                    0 => 0xFF,
                    1 => 0,
                    2 => 0x80,
                    _ => rnd() as u8,
                };
            }
            exercise(&g);
        }
    }
}
