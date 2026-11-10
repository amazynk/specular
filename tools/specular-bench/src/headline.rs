//! The instruction-count gate's headline cases (feature `gate`): the CPU work behind the
//! budgets of DESIGN.md §12, on generated inputs, one per process,
//! counted like the decode cases (only the `perfgate_measure*` functions; set-up outside them
//! is not counted).
//!
//! | case | the work | budget |
//! |---|---|---|
//! | `first-pixels-24mp` | a 24 MP JPEG's 1/8 decode, the first pixels | 15 ms of the < 50 ms first pixels (the rest is GPU start-up), 4 cores |
//! | `tiff-100k-first-view` | open a 100k × 100k tiled BigTIFF pyramid, decode its fit view | first view < 300 ms, 4 cores |
//! | `ct-512-first-slice` | scan a 512-slice CT series' headers, decode the opened slice | first slice < 500 ms, 1 core (the scan is serial) |
//! | `ct-512-volume` | assemble the 512 slices into the MPR volume | volume ready < 3 s, 4 cores |
//! | `map-cluster-50k` | cluster 50,000 photo positions | one "instant" response, 100 ms, 1 core |
//! | `map-frame-50k` | 10 map frames' markers and base-map tiles, zoom 2 to 18 | 10 frames at 60 fps, 1 core |
//! | `library-grid-100k` | lay out a 100k-item grid and scroll it for 120 frames | 121 frames at 60 fps, 1 core |
//! | `h264enc-720p` | encode 4 frames of 720p for a slideshow video (one slice) | none yet |
//! | `dicom-anon-64` | anonymise 64 CT slices (Basic Profile, fixed key) | none yet |
//!
//! The last two have no budget in DESIGN.md: they gate regressions only (`budget_ms` 0).
//! Budgets are turned into instructions by `tools/bench/perf-gate.py` (3 instructions per ns per
//! core).

use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};

/// A headline case: name, generated input, budget (ms, reference cores) and what the budget is.
pub struct Headline {
    pub name: &'static str,
    pub input: &'static str,
    pub budget_ms: f64,
    pub cores: u32,
    pub what: &'static str,
}

pub const CASES: &[Headline] = &[
    Headline {
        name: "first-pixels-24mp",
        input: "first-pixels-24mp.jpg",
        budget_ms: 15.0,
        cores: 4,
        what: "24 MP first pixels < 50 ms, less GPU start-up",
    },
    Headline {
        name: "tiff-100k-first-view",
        input: "tiff-100k.tif",
        budget_ms: 300.0,
        cores: 4,
        what: "100k x 100k TIFF first view < 300 ms",
    },
    Headline {
        name: "ct-512-first-slice",
        input: "ct-512.pack",
        budget_ms: 500.0,
        cores: 1,
        what: "512-slice CT series to first slice < 500 ms",
    },
    Headline {
        name: "ct-512-volume",
        input: "ct-512.pack",
        budget_ms: 3000.0,
        cores: 4,
        what: "512-slice CT volume ready < 3 s",
    },
    Headline {
        name: "map-cluster-50k",
        input: "map-50k.txt",
        budget_ms: 100.0,
        cores: 1,
        what: "50k photo positions clustered within an instant (100 ms)",
    },
    Headline {
        name: "map-frame-50k",
        input: "map-50k.txt",
        budget_ms: 10.0 * 1000.0 / 60.0,
        cores: 1,
        what: "10 map frames at 60 fps",
    },
    Headline {
        name: "library-grid-100k",
        input: "library-100k.txt",
        budget_ms: 121.0 * 1000.0 / 60.0,
        cores: 1,
        what: "100k-item grid laid out and scrolled 120 frames at 60 fps",
    },
    Headline {
        name: "h264enc-720p",
        input: "h264enc-720p.yuv",
        budget_ms: 0.0,
        cores: 1,
        what: "",
    },
    Headline {
        name: "dicom-anon-64",
        input: "ct-512.pack",
        budget_ms: 0.0,
        cores: 1,
        what: "",
    },
];

fn err<E: std::fmt::Debug>(e: E) -> anyhow::Error {
    anyhow!("{e:?}")
}

pub fn is_case(name: &str) -> bool {
    CASES.iter().any(|c| c.name == name)
}

/// Runs a headline case on `file` (set-up here, the counted work in `perfgate_measure_*`).
/// Returns what it did and the counted work's wall time in ms.
pub fn run(case: &str, file: &Path) -> Result<(String, f64)> {
    let mut ms = 0.0;
    let out = run_timed(case, file, &mut ms)?;
    Ok((out, ms))
}

/// Times `f` into `ms`.
fn timed<R>(ms: &mut f64, f: impl FnOnce() -> R) -> R {
    let t = Instant::now();
    let r = f();
    *ms = t.elapsed().as_secs_f64() * 1e3;
    r
}

fn run_timed(case: &str, file: &Path, ms: &mut f64) -> Result<String> {
    let data = std::fs::read(file).with_context(|| format!("reading {}", file.display()))?;
    match case {
        "first-pixels-24mp" => {
            let (w, h) = timed(ms, || perfgate_measure_jpeg_eighth(&data))?;
            Ok(format!("{w}x{h}"))
        }
        "tiff-100k-first-view" => {
            let (w, h) = timed(ms, || perfgate_measure_tiff_view(&data))?;
            Ok(format!("{w}x{h}"))
        }
        "ct-512-first-slice" | "ct-512-volume" => {
            let dir = unpack(file, &data)?;
            let first = dir.join("slice-0000.dcm");
            if case == "ct-512-first-slice" {
                let (n, w, h) = timed(ms, || perfgate_measure_ct_first(&first))?;
                Ok(format!("{n} slices, first {w}x{h}"))
            } else {
                // The scan is the first slice's case; this one starts from its result.
                let scan = specular_dicom::series::scan(
                    &first,
                    specular_dicom::series::ScanLimits::default(),
                    &AtomicBool::new(false),
                )
                .map_err(err)?;
                let series = scan.series.into_iter().next().context("no series")?;
                let dims = timed(ms, || perfgate_measure_ct_volume(&series))?;
                Ok(format!("volume {dims:?}"))
            }
        }
        "map-cluster-50k" => {
            let pts = map_points(&data)?;
            let n = timed(ms, || perfgate_measure_map_cluster(&pts));
            Ok(format!("{n} points"))
        }
        "map-frame-50k" => {
            let pts = map_points(&data)?;
            let clusters = specular_map::cluster::Clusters::new(pts.iter().copied());
            let n = timed(ms, || perfgate_measure_map_frames(&clusters));
            Ok(format!("{n} markers drawn"))
        }
        "h264enc-720p" => {
            let bytes = timed(ms, || perfgate_measure_h264enc(&data))?;
            Ok(format!("{bytes} bytes coded"))
        }
        "dicom-anon-64" => {
            let dir = unpack(file, &data)?;
            let files: Vec<Vec<u8>> = (0..64)
                .map(|k| std::fs::read(dir.join(format!("slice-{k:04}.dcm"))))
                .collect::<std::io::Result<_>>()?;
            let n = timed(ms, || perfgate_measure_anon(&files))?;
            Ok(format!("{n} bytes written"))
        }
        "library-grid-100k" => {
            let items = grid_items(&data)?;
            let n = timed(ms, || perfgate_measure_grid(&items));
            Ok(format!("{n} cells drawn"))
        }
        other => bail!("unknown case {other}"),
    }
}

// ---------------------------------------------------------------------------------------------
// The counted work.

#[inline(never)]
fn perfgate_measure_jpeg_eighth(data: &[u8]) -> Result<(u32, u32)> {
    use specular_jpeg::{DecodeOptions, Parallelism, Scale};
    let opts = DecodeOptions {
        scale: Scale::Eighth,
        parallelism: Parallelism::Single,
        ..DecodeOptions::default()
    };
    let d = black_box(specular_jpeg::decode(black_box(data), &opts).map_err(err)?);
    Ok((d.width, d.height))
}

/// The viewer's fit view of a 1920 x 1080 window: the scale whose image still covers the
/// window's height (1/64 for 100k), decoded whole (the app asks for the visible region,
/// which at fit is all of it).
#[inline(never)]
fn perfgate_measure_tiff_view(data: &[u8]) -> Result<(u32, u32)> {
    let data = black_box(data);
    let t = specular_tiff::Tiff::open(data).map_err(err)?;
    let opts = specular_tiff::DecodeOptions {
        scale: 64,
        parallel: false,
        ..Default::default()
    };
    let d = black_box(t.decode(data, 0, &opts).map_err(err)?);
    Ok((d.width, d.height))
}

/// Opening a slice of a series: the headers of its folder scanned and assembled into series,
/// then the opened slice read, parsed and decoded to modality values.
#[inline(never)]
fn perfgate_measure_ct_first(first: &Path) -> Result<(usize, u32, u32)> {
    let scan = specular_dicom::series::scan(
        black_box(first),
        specular_dicom::series::ScanLimits::default(),
        &AtomicBool::new(false),
    )
    .map_err(err)?;
    let n = scan.series.first().map_or(0, |s| s.items.len());
    let bytes = std::fs::read(first)?;
    let d = specular_dicom::Dicom::parse(&bytes).map_err(err)?;
    let f = black_box(d.modality_frame(&bytes, 0, None).map_err(err)?);
    Ok((n, f.frame.width, f.frame.height))
}

/// MPR's volume load (app/mpr/load.rs, serially): stack the series, plan the volume, then read,
/// parse and decode every slice into it.
#[inline(never)]
fn perfgate_measure_ct_volume(series: &specular_dicom::Series<PathBuf>) -> Result<[u32; 3]> {
    use specular_dicom::volume::{self, Plan, Volume, VoxelKind};
    let st = volume::stack(black_box(series)).map_err(err)?;
    let read = |p: &Path| -> Result<(Vec<u8>, specular_dicom::Dicom)> {
        let bytes = std::fs::read(p)?;
        let d = specular_dicom::Dicom::parse(&bytes).map_err(err)?;
        Ok((bytes, d))
    };
    let first = &st.slices[0];
    let (b0, d0) = read(&first.key)?;
    let f0 = d0.modality_frame(&b0, first.frame, None).map_err(err)?;
    let kind = VoxelKind::of(&f0);
    let (w, h) = (f0.frame.width, f0.frame.height);
    let sp = f0.calibration().spacing.context("no pixel spacing")?;
    let full = [w, h, st.slices.len() as u32];
    let plan = Plan::new(full, kind, sp.x.min(sp.y), st.spacing, 2048, 4 << 30).map_err(err)?;
    let g = st.planned(&plan, w, h, [sp.x, sp.y]);
    let mut vol = Volume::new(g, kind, plan.step_xy).map_err(err)?;
    vol.put_frame(0, &f0).map_err(err)?;
    for (k, s) in st.slices.iter().enumerate().skip(1) {
        let (b, d) = read(&s.key)?;
        let f = d.modality_frame(&b, s.frame, None).map_err(err)?;
        vol.put_frame(k, &f).map_err(err)?;
    }
    Ok(black_box(vol).geometry.dims)
}

#[inline(never)]
fn perfgate_measure_map_cluster(pts: &[(u32, (f64, f64))]) -> usize {
    let c = black_box(specular_map::cluster::Clusters::new(
        black_box(pts).iter().copied(),
    ));
    c.len()
}

/// Ten frames of the photo map at 1280 x 800, zooming from the world into a city: the
/// clusters on screen, decluttered, and the base map's tiles.
#[inline(never)]
fn perfgate_measure_map_frames(c: &specular_map::cluster::Clusters) -> usize {
    use specular_map::Camera;
    let mut drawn = 0;
    for i in 0..10u32 {
        let mut cam = Camera::new((1280.0, 800.0));
        cam.zoom = 2.0 + f64::from(i) * 16.0 / 9.0;
        cam.center = (0.53, 0.35);
        cam.clamp();
        let on = black_box(c.on_screen(&cam, 40.0));
        let markers: Vec<(u32, (f64, f64))> = on.iter().map(|(cl, p)| (cl.count, *p)).collect();
        let keep = specular_map::cluster::declutter(&markers, |n| {
            12.0 + 4.0 * f64::from(n.max(1)).log10()
        });
        drawn += keep.iter().enumerate().filter(|(i, k)| i == *k).count();
        let z = specular_map::basemap::level(&cam, 1.0, 18);
        drawn += black_box(specular_map::basemap::visible(&cam, z)).len();
    }
    drawn
}

/// A 100k-item justified grid (photos of mixed aspect in month groups) laid out at 1920 px,
/// then scrolled top to bottom in 120 frames: each frame finds its rows and items and the
/// sticky header, and walks the visible cells.
#[inline(never)]
fn perfgate_measure_grid(items: &[specular_thumbs::layout::Item]) -> usize {
    use specular_thumbs::layout::{Layout, Mode, Params};
    let p = Params {
        width: 1920.0,
        tile: 200.0,
        caption: 18.0,
        ..Params::default()
    };
    let l = black_box(Layout::build(black_box(items), &p, Mode::Justified));
    let view_h = 1080.0;
    let bottom = l.clamp_scroll(f32::MAX, view_h);
    let mut cells = 0;
    for f in 0..120u32 {
        let y = bottom * f as f32 / 119.0;
        let r = l.items_in(y, y + view_h);
        black_box(l.rows_in(y, y + view_h));
        black_box(l.sticky_header(y));
        for i in r {
            if let Some(c) = l.cells.get(i) {
                cells += usize::from(black_box(c.w) > 0.0);
            }
        }
    }
    cells
}

const ENC_W: u32 = 1280;
const ENC_H: u32 = 720;
const ENC_FRAMES: usize = 4;

/// Four 720p frames through the slideshow encoder (High profile defaults, qp 23, one slice
/// so the count does not depend on the thread count), the first an IDR.
#[inline(never)]
fn perfgate_measure_h264enc(yuv: &[u8]) -> Result<usize> {
    let mut cfg = specular_h264enc::Config::new(ENC_W, ENC_H, 30, 1, 23);
    cfg.slices = 1;
    let mut e = specular_h264enc::Encoder::new(cfg).map_err(err)?;
    let (y, c) = ((ENC_W * ENC_H) as usize, (ENC_W * ENC_H / 4) as usize);
    let mut bytes = 0;
    for k in 0..ENC_FRAMES {
        let f = &yuv[k * (y + 2 * c)..(k + 1) * (y + 2 * c)];
        let frame = specular_h264enc::Frame {
            y: &f[..y],
            u: &f[y..y + c],
            v: &f[y + c..],
            y_stride: ENC_W as usize,
            c_stride: ENC_W as usize / 2,
        };
        bytes += black_box(e.encode(&frame, k == 0)).data.len();
    }
    Ok(bytes)
}

/// The anonymised export's per-file work (app/anon_export.rs `anonymise_one`) for 64 slices.
#[inline(never)]
fn perfgate_measure_anon(files: &[Vec<u8>]) -> Result<usize> {
    use specular_dicom::anon::{Options, Request, Session, anonymise};
    let options = Options::default();
    let session = Session::with_key([7; 32]);
    let req = Request {
        options: &options,
        session: &session,
        masks: &[],
        allow_burned_in: false,
        cancel: None,
    };
    let mut n = 0;
    for f in files {
        n += black_box(anonymise(black_box(f), &req).map_err(err)?)
            .bytes
            .len();
    }
    Ok(n)
}

// ---------------------------------------------------------------------------------------------
// Inputs.

/// `ct-512.pack`: the phantom series as one file (count, then name length, name, data length,
/// data per slice), so the gate hashes one input; unpacked beside it before a run.
fn unpack(file: &Path, data: &[u8]) -> Result<PathBuf> {
    let dir = file.with_extension("d");
    let mut at = 0usize;
    let mut take = |n: usize| -> Result<&[u8]> {
        let s = data.get(at..at + n).context("truncated pack")?;
        at += n;
        Ok(s)
    };
    let u32le = |b: &[u8]| u32::from_le_bytes(b.try_into().expect("4 bytes")) as usize;
    let count = u32le(take(4)?);
    std::fs::create_dir_all(&dir)?;
    for _ in 0..count {
        let n = u32le(take(4)?);
        let name = String::from_utf8(take(n)?.to_vec())?;
        let len = u32le(take(4)?);
        let body = take(len)?;
        let p = dir.join(&name);
        if std::fs::metadata(&p).map(|m| m.len() as usize).ok() != Some(len) {
            std::fs::write(&p, body)?;
        }
    }
    Ok(dir)
}

/// Photo positions in world units: clustered around 40 "cities", from integers only.
fn map_points(data: &[u8]) -> Result<Vec<(u32, (f64, f64))>> {
    let n: u32 = std::str::from_utf8(data)?
        .split_whitespace()
        .next()
        .context("map-50k.txt: count")?
        .parse()?;
    let city = |c: u32| {
        let h = super::gate::hash(c, 7, 99);
        (
            f64::from(h & 0xffff) / 65536.0,
            0.25 + f64::from(h >> 16) / 65536.0 * 0.4,
        )
    };
    Ok((0..n)
        .map(|i| {
            let (cx, cy) = city(i % 40);
            let h = super::gate::hash(i, 3, 5);
            let dx = (f64::from(h & 0xfff) - 2048.0) / 2048.0 * 0.002;
            let dy = (f64::from(h >> 20) - 2048.0) / 2048.0 * 0.002;
            (i, (cx + dx, cy + dy))
        })
        .collect())
}

/// Grid items: aspect ratios of photos (3:2, 2:3, 4:3, 16:9, 1:1) in month groups of 50 to
/// 2,000 images.
fn grid_items(data: &[u8]) -> Result<Vec<specular_thumbs::layout::Item>> {
    let n: u32 = std::str::from_utf8(data)?
        .split_whitespace()
        .next()
        .context("library-100k.txt: count")?
        .parse()?;
    const ASPECTS: [f32; 5] = [1.5, 2.0 / 3.0, 4.0 / 3.0, 16.0 / 9.0, 1.0];
    let mut group = 0;
    let mut left = 0u32;
    Ok((0..n)
        .map(|i| {
            if left == 0 {
                group += 1;
                left = 50 + super::gate::hash(group, 1, 2) % 1950;
            }
            left -= 1;
            specular_thumbs::layout::Item {
                aspect: ASPECTS[(super::gate::hash(i, 9, 4) % 5) as usize],
                group,
            }
        })
        .collect())
}

/// Writes the headline inputs into `dir` (by `gate gen`).
pub fn generate(dir: &Path, photo: &[u8], pw: u32, ph: u32) -> Result<()> {
    super::gate::write(dir, "first-pixels-24mp.jpg", &jpeg_24mp(photo, pw, ph)?)?;
    super::gate::write(dir, "tiff-100k.tif", &tiff_100k(photo, pw, ph))?;
    super::gate::write(dir, "ct-512.pack", &ct_pack())?;
    super::gate::write(
        dir,
        "map-50k.txt",
        b"50000 photo positions around 40 cities\n",
    )?;
    super::gate::write(
        dir,
        "library-100k.txt",
        b"100000 grid items in month groups\n",
    )?;
    super::gate::write(dir, "h264enc-720p.yuv", &yuv_frames(photo, pw, ph))?;
    Ok(())
}

/// `ENC_FRAMES` I420 frames: 720p windows of the photo panning right, BT.709 limited range in
/// integer arithmetic.
fn yuv_frames(photo: &[u8], pw: u32, ph: u32) -> Vec<u8> {
    let (w, h) = (ENC_W, ENC_H);
    let mut out = Vec::new();
    for k in 0..ENC_FRAMES as u32 {
        let (ox, oy) = (k * 24, (ph - h) / 2);
        let px = |x: u32, y: u32| {
            let i = (((oy + y) * pw + (ox + x).min(pw - 1)) * 3) as usize;
            (
                i32::from(photo[i]),
                i32::from(photo[i + 1]),
                i32::from(photo[i + 2]),
            )
        };
        for y in 0..h {
            for x in 0..w {
                let (r, g, b) = px(x, y);
                out.push(((47 * r + 157 * g + 16 * b + 128) / 256 + 16) as u8);
            }
        }
        for plane in 0..2 {
            for y in (0..h).step_by(2) {
                for x in (0..w).step_by(2) {
                    let (r, g, b) = px(x, y);
                    let v = if plane == 0 {
                        (-26 * r - 87 * g + 112 * b + 128) / 256 + 128
                    } else {
                        (112 * r - 102 * g - 10 * b + 128) / 256 + 128
                    };
                    out.push(v.clamp(16, 240) as u8);
                }
            }
        }
    }
    out
}

/// 6144 x 4096 (25 MP) 4:2:0 JPEG, quality 90: the generated photo mirrored three times
/// across and down (a photo-like spectrum everywhere, unlike a plain repeat's seams).
fn jpeg_24mp(photo: &[u8], pw: u32, ph: u32) -> Result<Vec<u8>> {
    let (w, h) = (pw * 3, ph * 8 / 3);
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        let ty = y / ph;
        let sy = if ty.is_multiple_of(2) {
            y % ph
        } else {
            ph - 1 - y % ph
        };
        for x in 0..w {
            let tx = x / pw;
            let sx = if tx.is_multiple_of(2) {
                x % pw
            } else {
                pw - 1 - x % pw
            };
            let i = ((sy * pw + sx) * 3) as usize;
            rgb.extend_from_slice(&photo[i..i + 3]);
        }
    }
    let mut out = Vec::new();
    let mut e = jpeg_encoder::Encoder::new(&mut out, 90);
    e.set_sampling_factor(jpeg_encoder::SamplingFactor::R_4_2_0);
    e.encode(&rgb, w as u16, h as u16, jpeg_encoder::ColorType::Rgb)
        .map_err(err)?;
    Ok(out)
}

/// A 100,000 x 100,000 RGB8 BigTIFF, 256 x 256 deflate tiles, with a reduced-resolution IFD per
/// halving down to one tile (as whole-slide scanners write them). Every level's tiles share 16
/// distinct tile payloads (patches of the photo, picked by a hash of the tile's position), so
/// the file is a few MB; the tile tables (204k entries) are as large as a real file's.
fn tiff_100k(photo: &[u8], pw: u32, ph: u32) -> Vec<u8> {
    const SIDE: u32 = 100_000;
    const TILE: u32 = 256;
    let mut payloads: Vec<Vec<u8>> = Vec::new();
    for k in 0..16u32 {
        let ox = (k % 4) * (pw - TILE) / 3;
        let oy = (k / 4) * (ph - TILE) / 3;
        let mut t = Vec::with_capacity((TILE * TILE * 3) as usize);
        for y in 0..TILE {
            let i = (((oy + y) * pw + ox) * 3) as usize;
            t.extend_from_slice(&photo[i..i + (TILE * 3) as usize]);
        }
        payloads.push(miniz_oxide::deflate::compress_to_vec_zlib(&t, 6));
    }
    let mut levels = Vec::new();
    let mut side = SIDE;
    loop {
        levels.push(side);
        if side <= TILE {
            break;
        }
        side = side.div_ceil(2);
    }
    // Layout: header, payloads, then per level its two tables and its IFD.
    let mut out = Vec::new();
    out.extend_from_slice(b"II");
    out.extend_from_slice(&43u16.to_le_bytes());
    out.extend_from_slice(&8u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u64.to_le_bytes()); // first IFD, patched below
    let mut at = Vec::new();
    for p in &payloads {
        at.push(out.len() as u64);
        out.extend_from_slice(p);
    }
    let mut prev_next: usize = 8;
    for (li, &s) in levels.iter().enumerate() {
        let across = s.div_ceil(TILE);
        let n = (across * across) as usize;
        while out.len() % 8 != 0 {
            out.push(0);
        }
        let offsets_at = out.len() as u64;
        for i in 0..n as u32 {
            let k = (super::gate::hash(i % across, i / across, li as u32) % 16) as usize;
            out.extend_from_slice(&at[k].to_le_bytes());
        }
        let counts_at = out.len() as u64;
        for i in 0..n as u32 {
            let k = (super::gate::hash(i % across, i / across, li as u32) % 16) as usize;
            out.extend_from_slice(&(payloads[k].len() as u64).to_le_bytes());
        }
        while out.len() % 8 != 0 {
            out.push(0);
        }
        let ifd_at = out.len() as u64;
        out[prev_next..prev_next + 8].copy_from_slice(&ifd_at.to_le_bytes());
        // (tag, type, count, value): SHORT 3, LONG 4, LONG8 16.
        let entries: [(u16, u16, u64, u64); 12] = [
            (254, 4, 1, u64::from(li > 0)),
            (256, 4, 1, u64::from(s)),
            (257, 4, 1, u64::from(s)),
            // Three SHORTs fit in the 8-byte value field.
            (258, 3, 3, 8 | 8 << 16 | 8 << 32),
            (259, 3, 1, 8),
            (262, 3, 1, 2),
            (277, 3, 1, 3),
            (284, 3, 1, 1),
            (322, 3, 1, u64::from(TILE)),
            (323, 3, 1, u64::from(TILE)),
            (324, 16, n as u64, offsets_at),
            (325, 16, n as u64, counts_at),
        ];
        out.extend_from_slice(&(entries.len() as u64).to_le_bytes());
        for (tag, ty, count, value) in entries {
            out.extend_from_slice(&tag.to_le_bytes());
            out.extend_from_slice(&ty.to_le_bytes());
            out.extend_from_slice(&count.to_le_bytes());
            out.extend_from_slice(&value.to_le_bytes());
        }
        prev_next = out.len();
        out.extend_from_slice(&0u64.to_le_bytes());
    }
    out
}

/// The CT phantom as a 512-slice series of 256 x 256 slices (0.7 mm pixels, 0.8 mm apart).
fn ct_pack() -> Vec<u8> {
    let files = specular_dicom::volume::phantom::series([256, 256, 512], 0.7, 0.8, 0.0);
    let mut out = Vec::new();
    out.extend_from_slice(&(files.len() as u32).to_le_bytes());
    for (name, data) in files {
        out.extend_from_slice(&(name.len() as u32).to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&data);
    }
    out
}
