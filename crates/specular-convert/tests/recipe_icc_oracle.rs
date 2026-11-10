//! Export recipes' colour spaces against an ICC oracle: a chart of 1,000 sRGB colours is
//! exported as 8-bit PNG in sRGB, Display P3, Adobe RGB and ProPhoto with the recipe's own
//! profile. LittleCMS (Pillow's ImageCms, `gen/icc_oracle.py`) converts the chart from sRGB
//! into that profile: every sample must equal ours within 1 code (rounding). The way back to
//! sRGB is reported too (8-bit steps of a wide space cost a few codes near black). Skipped
//! when python3 with Pillow is missing.

#![allow(clippy::disallowed_methods)] // Tests may start unnamed processes.

use std::path::PathBuf;
use std::process::Command;

use specular_convert::Format;
use specular_convert::recipe::finish::{Linear, finish};
use specular_convert::recipe::space::Primaries;
use specular_convert::recipe::{Recipe, Space};

fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

#[test]
fn spaces_match_littlecms() {
    let ok = Command::new("python3")
        .args(["-I", "-c", "import PIL.ImageCms"])
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        eprintln!("python3 with Pillow not found: ICC oracle skipped");
        return;
    }
    let base = std::env::var_os("TMPDIR").map_or_else(std::env::temp_dir, PathBuf::from);
    let dir = base.join(format!("specular-recipe-icc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // 10 levels per channel, 40 × 25 px.
    let levels: Vec<u8> = (0..10).map(|i| (i * 255 / 9) as u8).collect();
    let mut srgb = Vec::new();
    for &r in &levels {
        for &g in &levels {
            for &b in &levels {
                srgb.extend([r, g, b]);
            }
        }
    }
    let (w, h) = (40u32, 25u32);
    let rgba: Vec<f32> = srgb
        .chunks_exact(3)
        .flat_map(|p| {
            let l = [p[0], p[1], p[2]].map(|v| srgb_to_linear(f32::from(v) / 255.0));
            [l[0], l[1], l[2], 1.0]
        })
        .collect();
    let img = Linear {
        width: w,
        height: h,
        rgba,
        primaries: Primaries::Bt709,
    };
    let reference = dir.join("reference.rgb");
    std::fs::write(&reference, &srgb).unwrap();
    let mut files = Vec::new();
    for s in Space::ALL {
        let r = Recipe {
            format: Format::Png,
            space: s,
            ..Recipe::default()
        };
        let f = finish(&img, (w, h), &r, None, None, None).unwrap();
        let p = dir.join(format!("{}.png", s.id()));
        std::fs::write(&p, &f.bytes).unwrap();
        files.push(p);
    }
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/gen/icc_oracle.py");
    let out = Command::new("python3")
        .arg("-I")
        .arg(script)
        .arg(&reference)
        .arg("1")
        .args(&files)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    let mut seen = 0;
    for line in text.lines() {
        eprintln!("{line}");
        let parts: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(parts.len(), 4, "{line}");
        let worst: u32 = parts[1].parse().unwrap();
        let over: u32 = parts[2].parse().unwrap();
        let back: u32 = parts[3].parse().unwrap();
        assert!(worst <= 1 && over == 0, "{line}");
        assert!(back <= 16, "{line}");
        seen += 1;
    }
    assert_eq!(seen, 4);
    let _ = std::fs::remove_dir_all(&dir);
}
