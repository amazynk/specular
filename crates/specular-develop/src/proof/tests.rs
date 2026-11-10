use wgpu::util::DeviceExt;

use super::*;
use crate::testgpu;
use specular_color::math::delta_e2000;

/// Linear Rec.709 → CIELAB (D50).
pub(crate) fn lab(rgb: [f32; 3]) -> [f32; 3] {
    let m = colour::mul(
        &colour::adaptation_xyz(
            colour::Cat::Bradford,
            colour::xy_to_xyz(colour::D65_XY),
            colour::xy_to_xyz(colour::D50_XY),
        ),
        &colour::rgb_to_xyz(&colour::REC709_PRIMARIES, colour::D65_XY),
    );
    xyz_to_lab(colour::mulv(&m, rgb.map(f64::from)).map(|v| v as f32))
}

fn p95(mut v: Vec<f32>) -> (f32, f32) {
    v.sort_by(f32::total_cmp);
    (v[v.len() * 95 / 100], v[v.len() - 1])
}

const ALL: [ProofSettings; 5] = [
    ProofSettings {
        intent: ProofIntent::Perceptual,
        bpc: false,
        paper: false,
        ink: false,
    },
    ProofSettings {
        intent: ProofIntent::Relative,
        bpc: false,
        paper: false,
        ink: false,
    },
    ProofSettings {
        intent: ProofIntent::Relative,
        bpc: true,
        paper: false,
        ink: false,
    },
    ProofSettings {
        intent: ProofIntent::Relative,
        bpc: true,
        paper: false,
        ink: true,
    },
    ProofSettings {
        intent: ProofIntent::Perceptual,
        bpc: false,
        paper: true,
        ink: true,
    },
];

#[test]
fn synthetic_profile_reads_and_behaves_like_paper() {
    let icc = synthetic::icc();
    assert_eq!(
        specular_color::icc::description(&icc).as_deref(),
        Some(synthetic::DESCRIPTION)
    );
    let p = Profile::parse(&icc).expect("parses");
    assert_eq!(&p.class, b"prtr");
    assert!(p.a2b[0].is_some() && p.a2b[1].is_some() && p.b2a[0].is_some() && p.b2a[1].is_some());
    // The table round-trips the model inside the gamut.
    for d in [[0.2, 0.5, 0.8], [0.9, 0.9, 0.9], [0.5, 0.1, 0.3]] {
        let x = synthetic::forward(d);
        let back = synthetic::inverse(x);
        for k in 0..3 {
            assert!((back[k] - d[k]).abs() < 1e-6, "{d:?} {back:?}");
        }
    }
    let rel = Proofer::new(&icc, ALL[2]).unwrap();
    let paper = Proofer::new(&icc, ALL[4]).unwrap();
    let ink = Proofer::new(&icc, ALL[3]).unwrap();
    // Relative with black point compensation keeps white and black.
    let w = rel.proof([1.0; 3]);
    assert!(w.iter().all(|&v| v > 0.98), "{w:?}");
    assert!(rel.proof([0.0; 3]).iter().all(|&v| v < 0.01));
    // The paper's white is warm and darker; the ink's black is a dark grey.
    let pw = paper.proof([1.0; 3]);
    assert!(pw[2] < pw[0] - 0.05 && pw[1] < 0.95, "{pw:?}");
    let ib = ink.proof([0.0; 3]);
    assert!(ib.iter().all(|&v| v > 0.01), "{ib:?}");
    // Greys are inside the gamut, a pure sRGB blue and green are not.
    assert!(rel.gamut_de([0.18; 3]) < 1.0);
    assert!(rel.gamut_de([0.0, 0.0, 1.0]) > GAMUT_DE);
    assert!(rel.gamut_de([0.0, 1.0, 0.0]) > GAMUT_DE);
    // Its primaries sit inside sRGB's on the vectorscope.
    let t = rel.targets().expect("RGB profile");
    let s = crate::scopes::targets();
    for k in 0..6 {
        let c = |a: [f32; 2]| (a[0] * a[0] + a[1] * a[1]).sqrt();
        assert!(c(t.ab[k]) < c(s.ab[k]), "{k}: {:?} {:?}", t.ab[k], s.ab[k]);
    }
}

#[test]
fn other_profiles_proof_or_refuse() {
    // A CMYK seed profile proofs; L*a*b* is not an output.
    let cmyk = include_bytes!("../../../specular-color/fuzz/seeds/profile/cmyk_v2_lut16.icc");
    let p = Proofer::new(cmyk, ProofSettings::default()).expect("CMYK proofs");
    assert!(p.targets().is_some());
    let _ = p.proof([0.5, 0.2, 0.1]);
    let lab = specular_color::profiles::lab();
    assert_eq!(
        Proofer::for_profile(&lab, ProofSettings::default()).unwrap_err(),
        ProofError::NotOutput
    );
    assert!(matches!(
        Proofer::new(b"not a profile", ProofSettings::default()),
        Err(ProofError::Unreadable(_))
    ));
}

#[test]
fn baked_table_follows_the_exact_chain() {
    let icc = synthetic::icc();
    let pic = crate::scopes::tests::picture(300, 200, 1.0);
    for s in ALL {
        let p = Proofer::new(&icc, s).unwrap();
        let lut = p.bake();
        let mut de = Vec::new();
        for px in pic.chunks_exact(4) {
            let rgb = [px[0], px[1], px[2]];
            let want = p.proof(rgb);
            let got = lut.eval(rgb);
            de.push(delta_e2000(lab(want), lab([got[0], got[1], got[2]])));
        }
        let (q, max) = p95(de);
        eprintln!("proof table {s:?}: ΔE2000 p95 {q:.3}, max {max:.3}");
        assert!(q <= 1.0, "{s:?}: p95 {q}");
    }
}

#[test]
fn gpu_matches_cpu_reference() {
    let Some(d) = testgpu::dev() else { return };
    let _g = testgpu::lock();
    let icc = synthetic::icc();
    let (w, h) = (640, 427);
    let pic = crate::scopes::tests::picture(w, h, 1.0);
    let mut pass = ProofPass::new(&d.device, &d.queue);
    for prec in [Precision::F32, Precision::F16] {
        for s in ALL {
            let p = Proofer::new(&icc, s).unwrap();
            let lut = p.bake();
            let bytes: Vec<u8> = match prec {
                Precision::F32 => bytemuck::cast_slice(&pic).to_vec(),
                Precision::F16 => pic
                    .iter()
                    .flat_map(|v| half::f16::from_f32(*v).to_bits().to_le_bytes())
                    .collect(),
            };
            let buf = d
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("proof test"),
                    contents: &bytes,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                });
            pass.run(&lut, &buf, w, h, prec);
            let out = read(&d.device, &d.queue, &buf, bytes.len() as u64);
            let got: Vec<f32> = match prec {
                Precision::F32 => bytemuck::cast_slice(&out).to_vec(),
                Precision::F16 => out
                    .chunks_exact(2)
                    .map(|c| half::f16::from_bits(u16::from_le_bytes([c[0], c[1]])).to_f32())
                    .collect(),
            };
            let mut de = Vec::new();
            let mut gamut_flip = 0usize;
            for (src, g) in pic.chunks_exact(4).zip(got.chunks_exact(4)) {
                let rgb = [src[0], src[1], src[2]];
                let rgb = match prec {
                    Precision::F32 => rgb,
                    Precision::F16 => rgb.map(|v| half::f16::from_f32(v).to_f32()),
                };
                de.push(delta_e2000(lab(p.proof(rgb)), lab([g[0], g[1], g[2]])));
                if (p.gamut_de(rgb) > GAMUT_DE) != (g[3] > GAMUT_DE) {
                    gamut_flip += 1;
                }
            }
            let n = de.len();
            let (q, max) = p95(de);
            eprintln!(
                "proof GPU {prec:?} {s:?}: ΔE2000 p95 {q:.3}, max {max:.3}; gamut flag differs {gamut_flip}/{n}"
            );
            assert!(q <= 1.0, "p95 {q}");
            assert!(
                gamut_flip * 50 < n,
                "gamut flags differ on {gamut_flip} of {n}"
            );
        }
    }
}

fn read(device: &wgpu::Device, queue: &wgpu::Queue, buf: &wgpu::Buffer, n: u64) -> Vec<u8> {
    let rb = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("proof read"),
        size: n,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_buffer_to_buffer(buf, 0, &rb, 0, n);
    queue.submit([enc.finish()]);
    rb.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll");
    let v = rb.slice(..).get_mapped_range().expect("mapped").to_vec();
    rb.unmap();
    v
}

#[test]
fn profiles_are_found_in_folders() {
    let dir = std::env::temp_dir().join(format!("dv19-icc-{}", std::process::id()));
    let sub = dir.join("vendor");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("printer.icc"), synthetic::icc()).unwrap();
    std::fs::write(dir.join("junk.icc"), b"not a profile at all, far too short").unwrap();
    std::fs::write(dir.join("notes.txt"), b"x").unwrap();
    let srgb = specular_color::profiles::matrix_srgb_curve_icc(
        &specular_color::profiles::srgb().matrix.unwrap(),
        "A display",
    );
    std::fs::write(dir.join("display.icm"), srgb).unwrap();
    let found = find_profiles(std::slice::from_ref(&dir));
    let _ = std::fs::remove_dir_all(&dir);
    let names: Vec<&str> = found.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, [synthetic::DESCRIPTION, "A display"], "{found:?}");
    assert!(found[0].printer && !found[1].printer);
}
