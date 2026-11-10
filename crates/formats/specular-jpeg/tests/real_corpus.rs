//! Correctness on the owner's real heavy photos, read in place from `tools/corpus/real/`
//! (never copied into the crate). Skipped when the files or djpeg are absent.

mod common;

use std::path::{Path, PathBuf};

use common::*;
use specular_jpeg::*;

fn corpus() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/real");
    [
        "goats-panorama-108mp-baseline-444.jpg",
        "erlaufsee-panorama-34mp-progressive-444.jpg",
    ]
    .iter()
    .map(|f| dir.join(f))
    .filter(|p| p.exists())
    .collect()
}

#[test]
fn real_photos_match_djpeg() {
    if !tools_available() {
        eprintln!("skipping: djpeg not found");
        return;
    }
    let files = corpus();
    if files.is_empty() {
        eprintln!("skipping: tools/corpus/real not present");
        return;
    }
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let data = std::fs::read(&path).unwrap();
        let info = read_info(&data).unwrap();
        eprintln!(
            "{name}: {}x{} {:?} restart {}",
            info.width, info.height, info.coding, info.restart_interval
        );

        // Full scale, full colour, against djpeg's default output.
        let auto = decode(&data, &DecodeOptions::default()).unwrap();
        assert!(!auto.truncated, "{name}");
        {
            let ours = planes_to_rgba(&auto);
            let refi = djpeg_file(&path, &["-dct", "int", "-ppm"]);
            let (max, mean) =
                diff_rgba_pnm(&ours, auto.width as usize, auto.height as usize, &refi);
            eprintln!("  full: max {max} mean {mean:.5}");
            assert!(max <= 2, "{name}: full max diff {max} (mean {mean:.5})");
        }
        let single = decode(
            &data,
            &DecodeOptions {
                parallelism: Parallelism::Single,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(auto == single, "{name}: Auto and Single decodes differ");
        drop(single);

        // Scaled decodes: full colour (4:4:4) against djpeg -scale.
        for (scale, frac) in [
            (Scale::Half, "1/2"),
            (Scale::Quarter, "1/4"),
            (Scale::Eighth, "1/8"),
        ] {
            let d = decode(
                &data,
                &DecodeOptions {
                    scale,
                    ..Default::default()
                },
            )
            .unwrap();
            let refs = djpeg_file(&path, &["-dct", "int", "-scale", frac, "-ppm"]);
            let (max, mean) = diff_rgba_pnm(
                &planes_to_rgba(&d),
                d.width as usize,
                d.height as usize,
                &refs,
            );
            eprintln!("  {frac}: max {max} mean {mean:.5}");
            assert!(max <= 3, "{name} {frac}: max diff {max} (mean {mean:.5})");
        }

        // Region decodes through an index equal crops of the full decode.
        let ix = build_index(&data).unwrap();
        // Whole-image decodes through the index (progressive: horizontal stripes of bounded
        // coefficient memory) equal plain ones.
        for scale in [Scale::Full, Scale::Quarter] {
            let o = DecodeOptions {
                scale,
                ..Default::default()
            };
            let d = decode_with_index(&data, &ix, &o).unwrap();
            let same = if scale == Scale::Full {
                d == auto
            } else {
                d == decode(&data, &o).unwrap()
            };
            assert!(same, "{name} {scale:?}: indexed whole-image decode differs");
        }
        let (w, h) = (info.width, info.height);
        for r in [
            Region {
                x: w / 2 - 256,
                y: h / 2 - 256,
                w: 512,
                h: 512,
            },
            Region {
                x: w - 700,
                y: h - 300,
                w: 2000,
                h: 2000,
            },
            Region {
                x: 0,
                y: 0,
                w: 97,
                h: 61,
            },
        ] {
            let o = DecodeOptions {
                region: Some(r),
                ..Default::default()
            };
            let d = decode_with_index(&data, &ix, &o).unwrap();
            let d2 = decode(&data, &o).unwrap();
            assert!(
                d == d2,
                "{name} {r:?}: indexed and unindexed region decodes differ"
            );
            for (fp, rp) in auto.planes.iter().zip(&d.planes) {
                for y in 0..rp.height as usize {
                    let a = &fp.data[(d.origin.1 as usize + y) * fp.stride + d.origin.0 as usize..]
                        [..rp.width as usize];
                    let b = &rp.data[y * rp.stride..][..rp.width as usize];
                    assert!(
                        a == b,
                        "{name} {r:?}: region row {y} differs from the full decode"
                    );
                }
            }
        }
    }
}
