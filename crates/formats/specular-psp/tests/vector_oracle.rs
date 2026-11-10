//! Vector layers drawn from their shapes against the composite image PaintShop Pro stored:
//! for every file in the PSP corpus (tools/corpus/fetch-editors-psp.sh, or
//! `$SPECULAR_PSP_CORPUS`) with visible vector layers and a lossless full-size composite, the
//! layers are composited and compared with it. One `ORACLE<TAB>PSP-vector<TAB>file<TAB>pass|fail`
//! line per file, then the mean difference and the share of pixels off by more than 24.
//! Ignored: run with `--ignored --nocapture`.

use specular_psp as sp;
use std::path::{Path, PathBuf};

fn corpus() -> Option<PathBuf> {
    let p = std::env::var_os("SPECULAR_PSP_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/editors/psp")
        });
    p.is_dir().then_some(p)
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut v: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    v.sort();
    for p in v {
        if p.is_dir() {
            files(&p, out);
        } else if p
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "psp" | "pspimage"))
        {
            out.push(p);
        }
    }
}

fn rgba(img: &sp::Image) -> Vec<[f32; 4]> {
    match img.format {
        sp::PixelFormat::Gray8 => img
            .data
            .iter()
            .map(|&g| [f32::from(g), f32::from(g), f32::from(g), 255.0])
            .collect(),
        sp::PixelFormat::Rgba8 => img
            .data
            .chunks_exact(4)
            .map(|p| [p[0], p[1], p[2], p[3]].map(f32::from))
            .collect(),
        sp::PixelFormat::Rgba16 => img
            .data
            .chunks_exact(8)
            .map(|p| {
                let v = |i: usize| f32::from(u16::from_ne_bytes([p[i], p[i + 1]])) / 257.0;
                [v(0), v(2), v(4), v(6)]
            })
            .collect(),
    }
}

#[test]
#[ignore = "oracle: needs the PSP corpus"]
fn vector_layers_match_the_stored_composite() {
    let Some(dir) = corpus() else {
        println!("no PSP corpus: skipped");
        return;
    };
    let mut list = Vec::new();
    files(&dir, &mut list);
    let dump = std::env::var_os("SPECULAR_PSP_DUMP").map(PathBuf::from);
    for p in &list {
        let name = p.strip_prefix(&dir).unwrap_or(p).display().to_string();
        let Ok(data) = std::fs::read(p) else { continue };
        let Ok(psp) = sp::parse(&data) else { continue };
        let kinds: Vec<String> = psp
            .layers()
            .iter()
            .map(|l| {
                format!(
                    "{:?}{}[{} op{} {:?} {:?}]",
                    l.kind,
                    if l.visible { "" } else { "(hidden)" },
                    l.name,
                    l.opacity,
                    l.blend,
                    l.rect
                )
            })
            .collect();
        println!(
            "# {name}: v{} composite {} layers {}",
            psp.info().major,
            psp.info().has_composite,
            kinds.join(" ")
        );
        let vector = psp
            .layers()
            .iter()
            .any(|l| l.visible && l.kind == sp::LayerKind::Vector);
        if !vector || !psp.info().has_composite {
            continue;
        }
        let stored = psp.render(
            &data,
            &sp::Options {
                prefer_composite: true,
                ..Default::default()
            },
        );
        let ours = psp.render(
            &data,
            &sp::Options {
                prefer_composite: false,
                ..Default::default()
            },
        );
        let (Ok(stored), Ok(ours)) = (stored, ours) else {
            println!("ORACLE\tPSP-vector\t{name}\tfail");
            continue;
        };
        if let Some(d) = &dump {
            let base = name.replace('/', "_");
            for (s, img) in [("ref", &stored), ("ours", &ours)] {
                let px = rgba(img);
                let mut v = format!(
                    "P7\nWIDTH {}\nHEIGHT {}\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n",
                    img.width, img.height
                )
                .into_bytes();
                v.extend(px.iter().flat_map(|p| p.map(|c| c.round() as u8)));
                let _ = std::fs::write(d.join(format!("{base}.{s}.pam")), v);
            }
        }
        let (a, b) = (rgba(&stored), rgba(&ours));
        let (mut sum, mut far) = (0f64, 0usize);
        for (p, q) in a.iter().zip(&b) {
            let pa = p[3] / 255.0;
            let qa = q[3] / 255.0;
            let mut worst = 0f32;
            for c in 0..3 {
                let d = (p[c] * pa - q[c] * qa).abs();
                worst = worst.max(d);
                sum += f64::from(d);
            }
            worst = worst.max((p[3] - q[3]).abs());
            far += usize::from(worst > 24.0);
        }
        let n = a.len().max(1);
        let mean = sum / (3 * n) as f64;
        let frac = far as f64 / n as f64;
        let ok = mean <= 1.5 && frac <= 0.005;
        println!(
            "ORACLE\tPSP-vector\t{name}\t{}",
            if ok { "pass" } else { "fail" }
        );
        println!("#   mean {mean:.2}, {:.2}% beyond 24", frac * 100.0);
    }
}
