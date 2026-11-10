//! Renders boards and schematics to PPM pictures for looking at by eye:
//!
//! `SPECULAR_KICAD_RENDER="a.kicad_pcb b.kicad_sch" SPECULAR_KICAD_RENDER_OUT=dir cargo test -p
//! specular-kicad --test render -- --ignored`

use std::path::{Path, PathBuf};

use specular_kicad::{Limits, pcb, sch};
use specular_vector::draw::{View, fit_transform};
use specular_vector::fonts::Fonts;
use specular_vector::prep::{PrepLimits, prepare};

#[test]
#[ignore]
fn render_to_ppm() {
    let (Some(files), Some(out)) = (
        std::env::var("SPECULAR_KICAD_RENDER").ok(),
        std::env::var_os("SPECULAR_KICAD_RENDER_OUT"),
    ) else {
        eprintln!("SPECULAR_KICAD_RENDER / SPECULAR_KICAD_RENDER_OUT not set");
        return;
    };
    let out = PathBuf::from(out);
    let dark = std::env::var_os("SPECULAR_KICAD_DARK").is_some();
    for f in files.split_whitespace() {
        let path = Path::new(f);
        let bytes = std::fs::read(path).expect("file");
        let dir = path.parent().unwrap().to_owned();
        let resolve = |n: &str| std::fs::read(dir.join(n)).ok();
        let t0 = std::time::Instant::now();
        let scene = if f.ends_with(".kicad_sch") {
            sch::scene(&bytes, &resolve, &Limits::default())
        } else {
            pcb::scene(&bytes, &Limits::default())
        }
        .expect("scene");
        let parse = t0.elapsed().as_secs_f64() * 1e3;
        let prepared = prepare(
            std::sync::Arc::new(scene),
            &Fonts::shared(),
            PrepLimits::default(),
        );
        let prep = t0.elapsed().as_secs_f64() * 1e3 - parse;
        println!("{f}: parse {parse:.0} ms, prepare {prep:.0} ms");
        for (i, sheet) in prepared.sheets.iter().enumerate().take(3) {
            let fit = sheet.fit.unwrap();
            let k = 1600.0 / fit.width().max(fit.height());
            let size = (
                (fit.width() * k) as u32 + 40,
                (fit.height() * k) as u32 + 40,
            );
            let layers: Vec<bool> = prepared.scene.layers.iter().map(|l| l.visible()).collect();
            let view = View {
                transform: fit_transform(fit, size, 20.0),
                size,
                dark,
                background: if dark { [33, 40, 48] } else { [255, 255, 255] },
                lineweights: false,
                dpi_scale: 1.0,
                layers: &layers,
            };
            let (px, _) = specular_vector::raster::render(sheet, &view).unwrap();
            let name = format!(
                "{}-{i}.ppm",
                path.file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .replace(' ', "_")
            );
            let mut ppm = format!("P6\n{} {}\n255\n", size.0, size.1).into_bytes();
            for p in px.chunks(4) {
                ppm.extend_from_slice(&p[..3]);
            }
            std::fs::write(out.join(name), ppm).unwrap();
        }
    }
}
