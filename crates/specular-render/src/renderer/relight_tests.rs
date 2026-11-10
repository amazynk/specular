//! The relight shader against [`Rti::shade`], its CPU reference, on the specular-rti fixtures
//! (every container, basis and colour layout) in every mode. GPU tests skip without an
//! adapter.

use std::path::{Path, PathBuf};

use specular_gfx::FULLSCREEN_WGSL;
use specular_rti::{Look, Mode, Rti};

use super::super::hdr_tests::{draw_with, gpu};
use super::*;
use crate::renderer::RenderOptions;

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

#[test]
fn relight_shader_validates() {
    let src = format!(
        "{FULLSCREEN_WGSL}\n{}",
        include_str!("../shaders/relight.wgsl")
    );
    let module = naga::front::wgsl::parse_str(&src)
        .unwrap_or_else(|e| panic!("relight: {}", e.emit_to_string(&src)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .unwrap_or_else(|e| panic!("relight: {e:?}"));
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../formats/specular-rti/tests/data")
}

fn open(path: &Path) -> Rti {
    if path.is_dir() {
        let info = std::fs::read(path.join("info.json")).expect("info.json");
        Rti::relight(&info, &mut |n| {
            std::fs::read(path.join(n)).map_err(|e| specular_rti::Error::Io(n.into(), e))
        })
        .expect("relight folder")
    } else {
        Rti::read(&std::fs::read(path).expect("fixture")).expect("rti")
    }
}

#[test]
fn plane_factor_fits_the_limits() {
    assert_eq!(plane_factor(4000, 3000, 9, 8192), 1);
    assert_eq!(plane_factor(10000, 3000, 1, 8192), 2);
    let f = plane_factor(8000, 8000, 32, 16384);
    let (w, h) = (8000u64.div_ceil(f as u64), 8000u64.div_ceil(f as u64));
    assert!(w * h * 4 * 32 <= PLANE_BUDGET);
}

/// Every fixture, every mode, four lights, drawn 1:1: the displayed colour within one 8-bit
/// step (plus rounding) of the CPU reference's clamped colour.
#[test]
fn gpu_relight_matches_the_reference() {
    let Some(gpu) = gpu() else {
        assert!(
            std::env::var_os("SPECULAR_REQUIRE_GPU").is_none(),
            "SPECULAR_REQUIRE_GPU is set but there is no GPU"
        );
        return;
    };
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let names = [
        "ptm_lrgb.ptm",
        "ptm_rgb.ptm",
        "ptm_jpeg_lrgb.ptm",
        "hsh2.rti",
        "hsh3.rti",
        "hsh4.rti",
        "relight_ptm_lrgb",
        "relight_hsh_rgb",
        "relight_sh_rgb",
        "relight_rbf_mrgb",
        "relight_bln_mycc",
    ];
    let lights = [
        Look::light_from_disk(0.0, 0.0),
        Look::light_from_disk(0.5, 0.3),
        Look::light_from_disk(-0.6, -0.2),
        Look::light_from_disk(0.1, -0.95),
    ];
    let (mut checked, mut worst) = (0usize, 0f32);
    let mut bad = Vec::new();
    for name in names {
        let rti = open(&fixtures().join(name));
        let mut img = renderer.create_image(&gpu, 7, rti.width, rti.height, None);
        assert!(renderer.set_relight(&gpu, &mut img, &rti), "{name}");
        assert!(img.is_relightable());
        let mut v = Vec::new();
        for mode in Mode::ALL {
            for light in lights {
                let look = Look {
                    light,
                    mode,
                    ..Look::default()
                };
                let p = rti.prepare(&look);
                assert!(renderer.relight(&gpu, &mut img, &p));
                let out = draw_with(&gpu, &mut renderer, &img, RenderOptions::default());
                for y in 0..rti.height {
                    for x in 0..rti.width {
                        rti.values(x, y, &mut v);
                        let want = Rti::shade(&p, &v);
                        let got = out[(y * rti.width + x) as usize];
                        for ch in 0..3 {
                            let w = want[ch].clamp(0.0, 1.0);
                            let g = linear_to_srgb(got[ch]);
                            let e = (g - w).abs();
                            worst = worst.max(e);
                            checked += 1;
                            if e > 1.5 / 255.0 {
                                bad.push(format!(
                                    "{name} {mode:?} {light:?} ({x}, {y}) ch {ch}: {g}, cpu {w}"
                                ));
                            }
                        }
                    }
                }
            }
        }
        renderer.recycle_image(img);
    }
    eprintln!(
        "relight GPU vs CPU: {checked} values, {} beyond 1.5/255, worst {:.5} ({:.2} steps)",
        bad.len(),
        worst,
        worst * 255.0
    );
    assert!(bad.is_empty(), "{:#?}", &bad[..bad.len().min(20)]);
}

/// Open and relight-frame times of a large file (`SPECULAR_RTI_TIMING` names a .ptm/.rti or
/// a Relight folder; skipped otherwise).
#[test]
fn relight_frame_time() {
    let Some(path) = std::env::var_os("SPECULAR_RTI_TIMING") else {
        return;
    };
    let Some(gpu) = gpu() else { return };
    let t = std::time::Instant::now();
    let rti = open(Path::new(&path));
    let open_ms = t.elapsed().as_secs_f64() * 1e3;
    let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
    let mut img = renderer.create_image(&gpu, 9, rti.width, rti.height, None);
    let t = std::time::Instant::now();
    assert!(renderer.set_relight(&gpu, &mut img, &rti));
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    let upload_ms = t.elapsed().as_secs_f64() * 1e3;
    let mut frames = Vec::new();
    let mut prep = Vec::new();
    for i in 0..40 {
        let a = i as f32 * 0.3;
        let look = Look {
            light: Look::light_from_disk(0.7 * a.cos(), 0.7 * a.sin()),
            ..Look::default()
        };
        let t = std::time::Instant::now();
        let p = rti.prepare(&look);
        prep.push(t.elapsed().as_secs_f64() * 1e3);
        assert!(renderer.relight(&gpu, &mut img, &p));
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        frames.push(t.elapsed().as_secs_f64() * 1e3);
    }
    frames.sort_by(f64::total_cmp);
    prep.sort_by(f64::total_cmp);
    eprintln!(
        "{}: {}×{} {} layers; open {open_ms:.0} ms, upload {upload_ms:.0} ms, relight frame \
         median {:.2} ms, p90 {:.2} ms, of which prepare {:.2} ms (texture {}×{})",
        Path::new(&path).display(),
        rti.width,
        rti.height,
        rti.layers.len(),
        frames[frames.len() / 2],
        frames[frames.len() * 9 / 10],
        prep[prep.len() / 2],
        img.relight.as_ref().map_or(0, |p| p.width),
        img.relight.as_ref().map_or(0, |p| p.height),
    );
}
