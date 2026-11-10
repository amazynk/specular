//! The GPU raw front end against `specular-raw`'s CPU path on synthetic DNGs (the DNG profile
//! with its look tables, the opcode lists 1 and 2, black deltas), and, as a developer-run
//! oracle, the sensor values against LibRaw (rawpy).
//!
//!     SPECULAR_REQUIRE_GPU=1 cargo test -p specular-develop --test raw_front_end
//!
//! The oracle (ignored by default; corpus from `tools/quality/fetch-raw-cameras.sh`):
//!
//!     SPECULAR_RAWPY=<venv>/bin/python SPECULAR_RAW_CAMERAS=<dir> SPECULAR_REQUIRE_GPU=1 \
//!       cargo test -p specular-develop --test raw_front_end -- --ignored --nocapture

#[path = "../../formats/specular-raw/tests/common/mod.rs"]
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use common::{
    DngCodec, DngSpec, OpParams, XYZ_TO_SRGB, demo_profile, op_gain_map, opcode_list, tag_floats,
    tag_longs, write_dng,
};
use specular_develop::graph::Precision;
use specular_develop::ops::raw::{
    FrontEnd, Options, Output, Quality, REC709_TO_REC2020, WhiteBalance,
};
use specular_raw::DecodeOptions;
use specular_raw::sensor::Layout;

struct Dev {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

fn dev() -> Option<&'static Dev> {
    static D: OnceLock<Option<Dev>> = OnceLock::new();
    D.get_or_init(|| match open() {
        Ok(d) => Some(d),
        Err(e) if std::env::var_os("SPECULAR_REQUIRE_GPU").is_none() => {
            eprintln!("no GPU ({e}): GPU tests skipped");
            None
        }
        Err(e) => panic!("SPECULAR_REQUIRE_GPU is set but there is no adapter: {e}"),
    })
    .as_ref()
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    L.lock().unwrap_or_else(|e| e.into_inner())
}

fn open() -> Result<Dev, String> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.flags = wgpu::InstanceFlags::empty().with_env();
    desc.backends = wgpu::Backends::PRIMARY.with_env();
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
        apply_limit_buckets: false,
    }))
    .map_err(|e| format!("no adapter: {e}"))?;
    let a = adapter.limits();
    let mut limits = wgpu::Limits::default().using_resolution(a.clone());
    limits.max_storage_buffer_binding_size = a.max_storage_buffer_binding_size;
    limits.max_buffer_size = a.max_buffer_size;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("raw front end tests"),
        required_features: wgpu::Features::empty(),
        required_limits: limits,
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::MemoryUsage,
        trace: wgpu::Trace::Off,
    }))
    .map_err(|e| format!("no device: {e}"))?;
    Ok(Dev { device, queue })
}

fn inverse(m: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let m = m.map(|r| r.map(f64::from));
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    let c = |a: usize, b: usize, c: usize, d: usize| m[a][b] * m[c][d] - m[a][d] * m[c][b];
    let inv = [
        [c(1, 1, 2, 2), -c(0, 1, 2, 2), c(0, 1, 1, 2)],
        [-c(1, 0, 2, 2), c(0, 0, 2, 2), -c(0, 0, 1, 2)],
        [c(1, 0, 2, 1), -c(0, 0, 2, 1), c(0, 0, 1, 1)],
    ];
    inv.map(|r| r.map(|v| (v / det) as f32))
}

fn gpu(file: &[u8], o: &Options) -> (u32, u32, Vec<f32>) {
    let d = dev().expect("a device");
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    let s = specular_raw::parse(file)
        .unwrap()
        .decode_sensor(file, &DecodeOptions::default())
        .unwrap();
    let g = fe.upload(s).unwrap();
    let out = fe.run(&g, o).unwrap();
    let v = fe.read(&out).unwrap();
    (out.width, out.height, v)
}

#[test]
fn dng_profile_tables_match_the_cpu_develop() {
    if dev().is_none() {
        return;
    }
    let _l = lock();
    // LinearRaw in linear sRGB primaries, each pixel another in-gamut colour.
    let (w, h) = (24u32, 16u32);
    let mut s = DngSpec::new(w, h, DngCodec::U16);
    s.linear_raw = true;
    s.color_matrix = XYZ_TO_SRGB;
    s.extra = demo_profile();
    let mut data = Vec::new();
    for y in 0..h {
        for x in 0..w {
            // Below half of white with no repeated top values: the CPU develop's clip
            // detection (a pile-up of equal top samples) stays out of it.
            let i = (y * w + x) as f32;
            let r = 0.05 + 0.4 * x as f32 / w as f32 + 1e-5 * i;
            let g = 0.05 + 0.4 * y as f32 / h as f32 + 2e-5 * i;
            let b = 0.05 + 0.4 * ((x * 7 + y * 3) % 11) as f32 / 11.0 + 3e-5 * i;
            data.extend([r, g, b].map(|v| (v * 65535.0).round() as u16));
        }
    }
    let file = write_dng(&s, &data);
    let cpu = specular_raw::parse(&file)
        .unwrap()
        .decode(&file, &DecodeOptions::default())
        .unwrap();
    assert_eq!(cpu.matrix_source, specular_raw::MatrixSource::DngProfile);
    let o = Options {
        precision: Precision::F32,
        ..Options::new(Quality::Full)
    };
    let (gw, gh, v) = gpu(&file, &o);
    assert_eq!((gw, gh), (cpu.width, cpu.height));
    let back = inverse(REC709_TO_REC2020);
    let mut worst = 0f32;
    for (i, p) in v.chunks_exact(4).enumerate() {
        // The CPU develop's 16-bit output cannot hold the negative components the look
        // tables' hue shifts make outside Rec.709; the GPU keeps them.
        let rec709: [f32; 3] = std::array::from_fn(|r| {
            (0..3)
                .map(|k| back[r][k] * p[k])
                .sum::<f32>()
                .clamp(0.0, 1.0)
        });
        for (c, got) in rec709.iter().enumerate() {
            let want = f32::from(cpu.data[i * 3 + c]) / 65535.0;
            worst = worst.max((got - want).abs());
        }
    }
    eprintln!("DNG profile: GPU vs CPU develop, max |diff| {worst:.2e}");
    assert!(worst < 5e-4, "profile differs by {worst}");
    // Without the tables, the matrix: a different rendering.
    let (_, _, m) = gpu(
        &file,
        &Options {
            profile: false,
            ..o
        },
    );
    assert!(m.iter().zip(&v).any(|(a, b)| (a - b).abs() > 1e-3));
}

fn bayer_dng(extra: Vec<common::Tag>) -> (Vec<u8>, Vec<u16>) {
    let (w, h) = (64u32, 48u32);
    let mut s = DngSpec::new(w, h, DngCodec::U16);
    s.black = 1000;
    s.white = 15000;
    s.extra = extra;
    let data: Vec<u16> = (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            (1200 + (x * 151 + y * 97) % 12000) as u16
        })
        .collect();
    (write_dng(&s, &data), data)
}

#[test]
fn opcode_lists_1_and_2_match_the_cpu_decode() {
    if dev().is_none() {
        return;
    }
    let _l = lock();
    let poly = (8, 0, {
        OpParams::default()
            .u32(&[0, 0, 48, 64, 0, 1, 1, 1, 2])
            .f64(&[0.0, 1.01, -0.005])
    });
    let gains: Vec<f32> = (0..12).map(|i| 0.9 + 0.02 * i as f32).collect();
    let bad = (4, 0, OpParams::default().u32(&[5000, 0]));
    let delta_col = (11, 0, {
        let mut p = OpParams::default().u32(&[0, 0, 48, 64, 0, 1, 1, 2, 32]);
        p = p.f32(&(0..32).map(|i| (i % 4) as f32 * 0.002).collect::<Vec<_>>());
        p
    });
    let (mut file, mut data) = bayer_dng(vec![
        opcode_list(1, vec![bad, poly]),
        opcode_list(
            2,
            vec![op_gain_map([0, 0, 48, 64], 1, 3, 4, &gains), delta_col],
        ),
    ]);
    // Two bad pixels (the constant 5000): the writer stored the samples uncompressed, so
    // patch the first two red samples of row 10 in place.
    let at = |d: &[u8], v: &[u16]| {
        d.windows(v.len() * 2)
            .position(|w| {
                w.chunks_exact(2)
                    .zip(v)
                    .all(|(c, &s)| u16::from_le_bytes([c[0], c[1]]) == s)
            })
            .unwrap()
    };
    let row = &data[10 * 64..11 * 64];
    let off = at(&file, row);
    for k in [4usize, 8] {
        file[off + k * 2..off + k * 2 + 2].copy_from_slice(&5000u16.to_le_bytes());
        data[10 * 64 + k] = 5000;
    }
    let raw = specular_raw::parse(&file)
        .unwrap()
        .decode_raw(&file, &DecodeOptions::default())
        .unwrap();
    assert_eq!(raw.white, 65535.0, "list 2 normalised the samples");
    let o = Options {
        precision: Precision::F32,
        wb: WhiteBalance::None,
        output: Output::Mosaic,
        ..Options::new(Quality::Full)
    };
    let (w, h, v) = gpu(&file, &o);
    let c = raw.crop;
    assert_eq!((w, h), (c.w, c.h));
    let mut worst = 0f32;
    for y in 0..h {
        for x in 0..w {
            let want = f32::from(raw.data[((c.y + y) * raw.width + c.x + x) as usize]) / 65535.0;
            let got = v[((y * w + x) * 4) as usize];
            worst = worst.max((got - want).abs());
        }
    }
    eprintln!("opcode lists 1 and 2: GPU vs CPU decode, max |diff| {worst:.2e}");
    assert!(worst <= 4.0 / 65535.0, "opcodes differ by {worst}");
}

#[test]
fn black_deltas_per_row_and_column() {
    if dev().is_none() {
        return;
    }
    let _l = lock();
    let cols: Vec<f32> = (0..64).map(|i| (i % 7) as f32 * 3.0).collect();
    let rows: Vec<f32> = (0..48).map(|i| (i % 5) as f32 * 2.0).collect();
    let (file, data) = bayer_dng(vec![
        tag_longs(0xc619, &[2, 2]),
        tag_floats(0xc61a, &[1000.0, 1010.0, 1020.0, 1030.0]),
        tag_floats(0xc61b, &cols),
        tag_floats(0xc61c, &rows),
    ]);
    let s = specular_raw::parse(&file)
        .unwrap()
        .decode_sensor(&file, &DecodeOptions::default())
        .unwrap();
    assert_eq!(s.meta.black.repeat, (2, 2));
    assert_eq!(s.meta.black.per_column, cols);
    assert_eq!(s.meta.black.per_row, rows);
    assert_eq!(s.samples, data);
    let o = Options {
        precision: Precision::F32,
        wb: WhiteBalance::None,
        output: Output::Mosaic,
        ..Options::new(Quality::Full)
    };
    let (w, h, v) = gpu(&file, &o);
    let mut worst = 0f32;
    for y in 0..h {
        for x in 0..w {
            let b = [1000.0, 1010.0, 1020.0, 1030.0][((y & 1) * 2 + (x & 1)) as usize]
                + cols[x as usize]
                + rows[y as usize];
            let want = (f32::from(data[(y * 64 + x) as usize]) - b) / (15000.0 - b);
            worst = worst.max((v[((y * w + x) * 4) as usize] - want).abs());
        }
    }
    assert!(worst < 1e-6, "black deltas off by {worst}");
}

// ---- Oracle: LibRaw ---------------------------------------------------------------------------

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, out);
        } else if !p
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        {
            out.push(p);
        }
    }
}

fn u32le(b: &[u8], i: usize) -> usize {
    u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap()) as usize
}

/// Sensor values against LibRaw's `raw_image` (bit-exact expected), then the GPU mosaic
/// (black, white, no opcodes, no white balance) against LibRaw's values normalised with
/// LibRaw's own black and white levels.
#[test]
#[ignore = "oracle: needs rawpy (SPECULAR_RAWPY) and a RAW corpus (SPECULAR_RAW_CAMERAS)"]
fn sensor_values_match_libraw() {
    let (Some(py), Some(dir)) = (
        std::env::var_os("SPECULAR_RAWPY"),
        std::env::var_os("SPECULAR_RAW_CAMERAS"),
    ) else {
        eprintln!("SPECULAR_RAWPY or SPECULAR_RAW_CAMERAS unset; skipping");
        return;
    };
    let _l = lock();
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/quality/rawpy_sensor.py");
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join("raw-oracle");
    std::fs::create_dir_all(&tmp).unwrap();
    let mut paths = Vec::new();
    files(Path::new(&dir), &mut paths);
    paths.sort();
    let (mut total_bad, mut total_n, mut cams) = (0u64, 0u64, 0);
    for p in &paths {
        let Ok(file) = std::fs::read(p) else { continue };
        let Ok(rf) = specular_raw::parse(&file) else {
            continue;
        };
        let s = match rf.decode_sensor(&file, &DecodeOptions::default()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{}: {e}", p.display());
                continue;
            }
        };
        let prefix = tmp.join("oracle");
        let out = Command::new(&py)
            .arg("-I")
            .arg(&script)
            .arg(p)
            .arg(&prefix)
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .output()
            .unwrap();
        if !out.status.success() {
            eprintln!("{}: rawpy failed", p.display());
            continue;
        }
        let facts: Vec<(String, String)> = String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .filter_map(|kv| kv.split_once('='))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let fact = |k: &str| {
            facts
                .iter()
                .find(|(a, _)| a == k)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        };
        let nums =
            |k: &str| -> Vec<f64> { fact(k).split(',').filter_map(|v| v.parse().ok()).collect() };
        let raw = std::fs::read(prefix.with_extension("raw")).unwrap();
        let (oh, ow, oc) = (u32le(&raw, 0), u32le(&raw, 1), u32le(&raw, 2));
        let od: Vec<u16> = raw[12..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        // LibRaw gives linear (demosaiced) DNG data as four channels of its own processed
        // image, not as stored: not comparable (as in specular-raw's raw_cameras oracle).
        if oc != s.cpp as usize {
            eprintln!(
                "{}: samples per pixel differ (ours {}, LibRaw {oc})",
                p.display(),
                s.cpp
            );
            continue;
        }
        // Same stored size: compare everything; else find the offset (masked borders).
        let (w, h, c) = (s.width as usize, s.height as usize, s.cpp as usize);
        let cmp = |dx: i64, dy: i64, step: usize| {
            let (mut bad, mut n) = (0u64, 0u64);
            for y in (0..h).step_by(step) {
                let oy = y as i64 + dy;
                if oy < 0 || oy as usize >= oh {
                    continue;
                }
                for x in 0..w {
                    let ox = x as i64 + dx;
                    if ox < 0 || ox as usize >= ow {
                        continue;
                    }
                    for k in 0..c {
                        n += 1;
                        let a = s.samples[(y * w + x) * c + k];
                        let b = od[(oy as usize * ow + ox as usize) * c + k];
                        bad += u64::from(a != b);
                    }
                }
            }
            (bad, n)
        };
        let (dx, dy) = if (w, h) == (ow, oh) {
            (0, 0)
        } else {
            let (rx, ry) = ((w as i64 - ow as i64).abs(), (h as i64 - oh as i64).abs());
            let mut best = (u64::MAX, 0, 0);
            for dy in -ry.min(64)..=ry.min(64) {
                for dx in -rx.min(64)..=rx.min(64) {
                    let (b, n) = cmp(dx, dy, 61);
                    if n > 0 && b < best.0 {
                        best = (b, dx, dy);
                    }
                }
            }
            (best.1, best.2)
        };
        let (bad, n) = cmp(dx, dy, 1);
        total_bad += bad;
        total_n += n;
        cams += 1;
        // The GPU mosaic, normalised with LibRaw's levels for comparison.
        let mut gpu_note = String::from("-");
        if matches!(s.meta.layout, Layout::Bayer(_) | Layout::Pattern(_)) {
            let black = nums("black");
            let white = fact("white").parse::<f64>().unwrap_or(0.0);
            let pat: Vec<usize> = fact("pattern")
                .split(',')
                .filter_map(|v| v.parse().ok())
                .collect();
            let pw: usize = fact("pattern_w").parse().unwrap_or(0);
            if black.len() >= 4 && white > 0.0 && pw > 0 && !pat.is_empty() {
                let o = Options {
                    precision: Precision::F32,
                    wb: WhiteBalance::None,
                    output: Output::Mosaic,
                    opcodes: false,
                    ..Options::new(Quality::Full)
                };
                let (gw, gh, v) = gpu(&file, &o);
                let cr = s.crop;
                let ph = pat.len() / pw;
                let (mut worst, mut over) = (0f64, 0u64);
                for y in 0..gh {
                    for x in 0..gw {
                        let (sx, sy) = ((cr.x + x) as i64 + dx, (cr.y + y) as i64 + dy);
                        if sx < 0 || sy < 0 || sx as usize >= ow || sy as usize >= oh {
                            continue;
                        }
                        let (sx, sy) = (sx as usize, sy as usize);
                        let ci = pat[(sy % ph) * pw + sx % pw];
                        let b = black[ci.min(3)];
                        let lv = f64::from(od[sy * ow + sx]);
                        let want = (lv - b) / (white - b);
                        let got = f64::from(v[((y * gw + x) * 4) as usize]);
                        let d = (got - want).abs();
                        worst = worst.max(d);
                        over += u64::from(d > 1e-3);
                    }
                }
                gpu_note = format!(
                    "GPU mosaic vs LibRaw levels: max {worst:.1e}, {over} over 1e-3 \
                     (white {} vs LibRaw {white}, black {:?} vs {:?})",
                    s.meta.white,
                    s.meta.black.levels,
                    &black[..4]
                );
            }
        }
        eprintln!(
            "{}: {bad} of {n} samples differ (offset {dx},{dy}); {gpu_note}",
            p.strip_prefix(&dir).unwrap_or(p).display()
        );
    }
    eprintln!("{cams} files: {total_bad} of {total_n} samples differ from LibRaw");
    assert_eq!(total_bad, 0, "sensor values differ from LibRaw");
}

/// `InputProfile::File`: a camera profile file (with a HueSatMap) on a DNG without one; the
/// GPU front end equals the CPU `ProfileTransform::apply` of the applied profile, and a
/// missing file falls back to the matrix with a reason.
#[test]
fn profile_file_on_the_gpu_matches_the_cpu_apply() {
    use specular_develop::model::params::{InputProfile, RawFrontEnd};
    use specular_develop::ops::raw::profile_file::Problem;
    use specular_raw::sensor::HsvTableData;
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (24u32, 16u32);
    let mut s = DngSpec::new(w, h, DngCodec::U16);
    s.linear_raw = true;
    s.color_matrix = XYZ_TO_SRGB;
    let mut data = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let r = 0.05 + 0.4 * x as f32 / w as f32;
            let g = 0.05 + 0.4 * y as f32 / h as f32;
            let b = 0.05 + 0.4 * ((x * 7 + y * 3) % 11) as f32 / 11.0;
            data.extend([r, g, b].map(|v| (v * 65535.0).round() as u16));
        }
    }
    let file = write_dng(&s, &data);
    let table = HsvTableData {
        hue: 6,
        sat: 3,
        val: 1,
        srgb: false,
        data: (0..18)
            .map(|i| {
                if i % 3 == 0 {
                    [0.0, 1.0, 1.0]
                } else {
                    [
                        (i % 5) as f32 * 2.0 - 4.0,
                        1.0 + 0.04 * (i % 4) as f32,
                        0.97,
                    ]
                }
            })
            .collect(),
    };
    let dcp = specular_raw::Dcp {
        camera_model: "Test".into(),
        name: "Chart".into(),
        copyright: String::new(),
        embed_policy: 3,
        illuminants: [21, 0],
        color_matrix: [
            Some([0.9, -0.3, -0.1, -0.4, 1.2, 0.2, -0.05, 0.15, 0.6]),
            None,
        ],
        forward_matrix: [
            Some([0.71, 0.15, 0.1042, 0.29, 0.74, -0.03, 0.02, -0.12, 0.92521]),
            None,
        ],
        hue_sat_map: [Some(table), None],
        look_table: None,
    };
    let dir = std::env::temp_dir().join(format!("specular-dcp-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("chart.dcp");
    std::fs::write(&path, dcp.write()).unwrap();
    let p = RawFrontEnd {
        input_profile: InputProfile::File,
        profile_path: path.to_string_lossy().into_owned(),
        ..RawFrontEnd::default()
    };
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    let sensor = specular_raw::parse(&file)
        .unwrap()
        .decode_sensor(&file, &DecodeOptions::default())
        .unwrap();
    let mut g = fe.upload(sensor).unwrap();
    let base = Options {
        precision: Precision::F32,
        ..Options::from_params(&p, 1.0)
    };
    assert!(base.profile);
    // The camera values, then the profile file's rendering.
    let cam = {
        let o = Options {
            output: Output::CameraRgb,
            ..base
        };
        let out = fe.run(&g, &o).unwrap();
        fe.read(&out).unwrap()
    };
    assert_eq!(g.use_input_profile(&p), None);
    let prof = g.sensor().meta.profile.clone().expect("the file's profile");
    let out = fe.run(&g, &base).unwrap();
    let v = fe.read(&out).unwrap();
    // In Rec.709 clipped to 0–1, as the DNG profile test compares: the CPU apply clamps
    // colours outside ProPhoto's 0–1 that the GPU keeps.
    let back = inverse(REC709_TO_REC2020);
    let mut worst = 0f32;
    for (c, p) in cam.chunks_exact(4).zip(v.chunks_exact(4)) {
        let want = prof.apply([c[0], c[1], c[2]]).map(|x| x.clamp(0.0, 1.0));
        let got: [f32; 3] = std::array::from_fn(|r| {
            (0..3)
                .map(|k| back[r][k] * p[k])
                .sum::<f32>()
                .clamp(0.0, 1.0)
        });
        for k in 0..3 {
            worst = worst.max((got[k] - want[k]).abs());
        }
    }
    eprintln!("profile file: GPU vs CPU apply, max |diff| {worst:.2e}");
    assert!(worst < 5e-4, "{worst}");
    // Back to the file's own colour, then a missing file: the matrix, with the reason.
    let matrix = RawFrontEnd {
        input_profile: InputProfile::Matrix,
        ..p.clone()
    };
    assert_eq!(g.use_input_profile(&matrix), None);
    assert!(g.sensor().meta.profile.is_none());
    let gone = RawFrontEnd {
        profile_path: dir.join("gone.dcp").to_string_lossy().into_owned(),
        ..p
    };
    assert_eq!(g.use_input_profile(&gone), Some(Problem::Missing));
    assert!(g.sensor().meta.profile.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
