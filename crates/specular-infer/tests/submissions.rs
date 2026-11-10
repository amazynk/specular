//! A run in short GPU submissions (`gpu::SUBMIT_MS`): the same bits as one submission,
//! and, ignored, the longest submission of every Local AI network before and after.
//!
//! ```text
//! SPECULAR_OCR_DIR=… SPECULAR_TESS_LINES=… SPECULAR_FACE_MODELS=… SPECULAR_SIGLIP2_DIR=… \
//! SPECULAR_PECORE_DIR=… cargo test -p specular-infer --test submissions -- --ignored --nocapture
//! ```
//! Networks without a local model file run with fixed-seed random weights at their real
//! size (the work does not depend on the weights' values).

mod common;

use std::path::PathBuf;

use common::contexts;
use specular_infer::depth::{self, DepthConfig};
use specular_infer::ffdnet::{self, FfdConfig};
use specular_infer::gpu::{Context, GpuGraph};
use specular_infer::models::{self, RandomWeights, VitConfig};
use specular_infer::tensor::Rng;
use specular_infer::{DType, Feed, Graph, Tensor, lama, sam, upernet, whisper};

fn rw(seed: u64) -> RandomWeights {
    RandomWeights { seed, f16: true }
}

/// A Tesseract-like stack: two LSTMs (one reversed, with lengths) over a batch of lines.
fn lstm_graph(r: &mut Rng) -> Graph {
    let (s, t, ns) = (4, 23, 40);
    let mut g = Graph::new();
    let pre = g.input(&[s, t, 4 * ns], DType::F32).unwrap();
    let lens = g.input(&[s], DType::F32).unwrap();
    let mut wh = || {
        Tensor::f32(
            &[ns, 4 * ns],
            (0..4 * ns * ns).map(|_| r.unit() * 0.4 - 0.2).collect(),
        )
    };
    let (w1, w2) = (wh(), wh());
    let w1 = g.constant(w1).unwrap();
    let w2 = g.constant(w2).unwrap();
    let y = g.lstm(pre, w1, Some((lens, 1)), false, false).unwrap();
    let y = g.unary(y, specular_infer::Act::Identity, 3.0, 0.0).unwrap();
    let y = g.concat(&[y, y, y, y], 2).unwrap();
    let y = g.lstm(y, w2, Some((lens, 1)), true, false).unwrap();
    g.output(y).unwrap();
    g
}

/// Runs `g` as one submission, as one submission per dispatch and with the default limit:
/// the outputs bit for bit the same.
fn same_bits(ctx: &Context, name: &str, g: &Graph, feeds: &[Feed]) {
    let mut gg = GpuGraph::new(ctx, g).unwrap();
    gg.set_submission_ms(f64::INFINITY);
    let one = gg.run(feeds).unwrap();
    assert_eq!(gg.last_run().submissions, 1, "{name}");
    gg.set_submission_ms(0.0);
    let each = gg.run(feeds).unwrap();
    assert_eq!(gg.last_run().submissions, gg.dispatches(), "{name}");
    let mut def = GpuGraph::new(ctx, g).unwrap();
    let dflt = def.run(feeds).unwrap();
    for (k, other) in [&each, &dflt].into_iter().enumerate() {
        assert_eq!(one.len(), other.len());
        for (a, b) in one.iter().zip(other) {
            assert_eq!(a.to_bytes(), b.to_bytes(), "{name}: run {k} differs");
        }
    }
    eprintln!(
        "{name}: {} dispatches, default limit: {} submissions",
        gg.dispatches(),
        def.last_run().submissions
    );
}

#[test]
fn split_submissions_change_no_bit() {
    let mut r = Rng::new(7);
    for ctx in contexts() {
        // Big-LaMa (Fourier units, transposed convolutions) on a small crop.
        let s = 64;
        let g = lama::graph(&mut rw(27), (s, s)).unwrap();
        let x = r.tensor(&[1, 4, s, s], 0.5);
        same_bits(ctx, "lama 64²", &g, &[Feed::Tensor(&x)]);
        // A ViT (attention, layer norms).
        let mut g = Graph::new();
        let c = VitConfig::tiny();
        let x = g.input(&[1, 3, c.image, c.image], DType::F32).unwrap();
        let y = models::vit(&mut g, x, &c, &mut rw(3)).unwrap();
        g.output(y).unwrap();
        let x = r.tensor(&[1, 3, c.image, c.image], 1.0);
        same_bits(ctx, "vit tiny", &g, &[Feed::Tensor(&x)]);
        // LSTM time steps (one dispatch each).
        let g = lstm_graph(&mut r);
        let pre = r.tensor(&[4, 23, 160], 1.0);
        let lens = Tensor::f32(&[4], vec![23.0, 9.0, 1.0, 17.0]);
        same_bits(ctx, "lstm", &g, &[Feed::Tensor(&pre), Feed::Tensor(&lens)]);
    }
}

/// The learnt estimates keep submissions near the limit: after a few runs of a graph
/// whose whole run is several times the limit, no submission is estimated over it unless
/// it is a single dispatch.
#[test]
fn estimates_follow_the_measured_times() {
    let Some(ctx) = contexts().first() else {
        return;
    };
    let s = 96;
    let g = lama::graph(&mut rw(27), (s, s)).unwrap();
    let x = Rng::new(3).tensor(&[1, 4, s, s], 0.5);
    let mut gg = GpuGraph::new(ctx, &g).unwrap();
    // The whole run's time from a split run (one submission of it could hold the GPU long
    // enough for a reset when other work shares it).
    gg.run(&[Feed::Tensor(&x)]).unwrap();
    let whole = gg.last_run().total_ms;
    // A limit of a quarter of the whole run.
    let limit = whole / 4.0;
    gg.set_submission_ms(limit);
    for _ in 0..3 {
        gg.run(&[Feed::Tensor(&x)]).unwrap();
    }
    let st = gg.last_run();
    eprintln!("lama 96²: whole {whole:.1} ms, limit {limit:.1}: {st:?}");
    assert!(st.submissions >= 3, "{st:?}");
    assert!(st.estimated_longest_ms <= limit * 1.01 || st.submissions > 1);
}

fn dir(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from)
}

/// One network's longest submission (GPU timestamps) with the default limit: the first run
/// (dispatch times not learnt yet) and the longest of the 3 after it. "One submission" is
/// a run's GPU time added up, what the whole run in one submission held: running it so
/// lost the device under load (EfficientSAM, 2026-11-02), so it is not run that way. Every
/// run's statistics come from the context, so networks that run several graphs (OCR,
/// speech) count them all.
fn report(ctx: &Context, name: &str, mut run: impl FnMut()) {
    ctx.set_submission_ms(specular_infer::gpu::SUBMIT_MS);
    let _ = ctx.take_stats();
    run();
    let first = ctx.take_stats();
    let (mut after, mut subs, mut whole) = (0f64, 0, first.longest_run_ms);
    for _ in 0..3 {
        run();
        let t = ctx.take_stats();
        after = after.max(t.longest_ms);
        subs = subs.max(t.submissions);
        whole = whole.max(t.longest_run_ms);
    }
    eprintln!(
        "| {name} | {} | {whole:.0} | {:.0} ({}) | {after:.0} ({subs}) |",
        first.runs, first.longest_ms, first.submissions
    );
}

fn graph_runner<'a>(ctx: &Context, g: &Graph, feeds: &'a [Feed<'a>]) -> impl FnMut() + 'a {
    let mut gg = GpuGraph::new(ctx, g).unwrap();
    move || {
        gg.run(feeds).unwrap();
    }
}

#[test]
#[ignore]
fn longest_submission_per_network() {
    // A device with timestamp queries: submissions timed on the GPU itself (wall times
    // count other processes' work under load).
    let ctx = &timestamp_context();
    assert!(
        ctx.features().timestamps,
        "the report needs timestamp queries"
    );
    let mut r = Rng::new(11);
    eprintln!(
        "load {:?}\n| network | graph runs | one submission ms | first split run ms (submissions) | later runs ms (submissions) |",
        std::fs::read_to_string("/proc/loadavg").ok()
    );
    // Remove: Big-LaMa on the largest crop.
    let s = 448;
    let g = lama::graph(&mut rw(27), (s, s)).unwrap();
    let x = r.tensor(&[1, 4, s, s], 0.5);
    report(
        ctx,
        "remove (Big-LaMa 448²)",
        graph_runner(ctx, &g, &[Feed::Tensor(&x)]),
    );
    drop(g);
    // Depth: Depth Anything V2 Small on a 3:2 photo.
    let (iw, ih) = depth::input_size(6000, 4000, depth::SIDE);
    let g = depth::graph_from_tensor(&mut rw(28), &DepthConfig::small(), (iw, ih)).unwrap();
    let x = r.tensor(&[1, 3, ih, iw], 1.0);
    report(
        ctx,
        "depth (DA V2 S)",
        graph_runner(ctx, &g, &[Feed::Tensor(&x)]),
    );
    // Denoise: one FFDNet tile.
    let t = specular_infer::tiles::Tiling::FFDNET;
    let g = ffdnet::graph(&mut rw(24), &FfdConfig::color(), (t.size, t.size)).unwrap();
    let x = r.tensor(&[1, 3, t.size, t.size], 0.5);
    let sigma = Tensor::f32(&[1, 1, 1, 1], vec![0.03]);
    report(
        ctx,
        "denoise (FFDNet tile)",
        graph_runner(ctx, &g, &[Feed::Tensor(&x), Feed::Tensor(&sigma)]),
    );
    // Masks: EfficientSAM ViT-S encoder, UperNet ConvNeXt-T (sky).
    let img: Vec<u8> = (0..1024 * 768 * 4).map(|i| (i * 7 % 251) as u8).collect();
    let feed = [Feed::Rgba {
        width: 1024,
        height: 768,
        data: &img,
    }];
    let g = sam::encoder(&mut rw(5), sam::SamConfig::from_width(384).unwrap(), true).unwrap();
    report(
        ctx,
        "masks (EfficientSAM S encoder)",
        graph_runner(ctx, &g, &feed),
    );
    let g = upernet::graph(
        &mut rw(6),
        &upernet::UperConfig::tiny(),
        upernet::input_size(1024, 768, 384),
        &[upernet::class::SKY],
    )
    .unwrap();
    report(
        ctx,
        "masks (UperNet ConvNeXt-T sky)",
        graph_runner(ctx, &g, &feed),
    );
    // Search: SigLIP 2 B/16-256 and PE Core S16-384 image towers.
    for (name, file, pe) in [
        (
            "search (SigLIP 2 B/16 image)",
            dir("SPECULAR_SIGLIP2_DIR").map(|d| d.join("siglip2-b16-256/model-f16.safetensors")),
            false,
        ),
        (
            "search (PE Core S16 image)",
            dir("SPECULAR_PECORE_DIR").map(|d| d.join("model-f16.safetensors")),
            true,
        ),
    ] {
        let Some(mut st) = file
            .filter(|f| f.is_file())
            .and_then(|f| specular_infer::safetensors::SafeTensorsFile::open(&f).ok())
        else {
            eprintln!("| {name} | not there |");
            continue;
        };
        let c = if pe {
            specular_infer::pecore::detect(&st.header).unwrap()
        } else {
            specular_infer::siglip::detect(&st.header).unwrap()
        };
        let mut g = Graph::new();
        let x = g.input(&[1, 3, c.image, c.image], DType::F32).unwrap();
        let y = if pe {
            specular_infer::pecore::image_tower(&mut g, x, &c, &mut st, true).unwrap()
        } else {
            specular_infer::siglip::image_tower(&mut g, x, &c, &mut st, true).unwrap()
        };
        g.output(y).unwrap();
        let x = r.tensor(&[1, 3, c.image, c.image], 1.0);
        report(ctx, name, graph_runner(ctx, &g, &[Feed::Tensor(&x)]));
    }
    // Faces: BlazeFace and the glintr100 embedder (as the People worker builds them).
    if let Some(m) = dir("SPECULAR_FACE_MODELS") {
        use specular_infer::face::blazeface::{Detector, Range};
        use specular_infer::face::embed::{Embedder, Kind};
        if let Ok(b) = std::fs::read(m.join("blaze_face_full_range.tflite")) {
            let mut det = Detector::new(&b, Range::Full, Some(ctx), 4).unwrap();
            let px: Vec<u8> = (0..640 * 480 * 4).map(|i| (i * 13 % 253) as u8).collect();
            let im = specular_infer::face::Image::new(640, 480, &px).unwrap();
            report(ctx, "faces (BlazeFace full)", || {
                det.detect(im).unwrap();
            });
        }
        if let Ok(b) = std::fs::read(m.join("glintr100.onnx")) {
            let mut e = Embedder::new(&b, Kind::AuraFace, true, Some(ctx), 4).unwrap();
            let x = r.tensor(&[1, 3, 112, 112], 1.0);
            report(ctx, "faces (glintr100 embedding)", || {
                e.raw(&x, false).unwrap();
            });
        }
    }
    // OCR: PP-OCRv5 detector and recogniser, Tesseract (Hebrew lines).
    if let Some(o) = dir("SPECULAR_OCR_DIR") {
        use specular_infer::ocr::lines::Rgba;
        use specular_infer::ocr::{Detector, PaddleNet, Recognizer, Runner};
        let runner = Runner::Gpu(ctx.clone());
        let det = o.join("PP-OCRv5_mobile_det_safetensors");
        let rec = o.join("latin_PP-OCRv5_mobile_rec_safetensors");
        if det.is_dir() && rec.is_dir() {
            let mut d = Detector::new(
                PaddleNet::open(
                    &det.join("config.json"),
                    &det.join("model.safetensors"),
                    true,
                )
                .unwrap(),
            )
            .unwrap();
            let (w, h) = (1600usize, 1200usize);
            let px: Vec<u8> = (0..w * h * 4)
                .map(|i| {
                    if (i / 4 / w / 9) % 3 == 0 && (i / 4 % w / 5) % 2 == 0 {
                        20
                    } else {
                        235
                    }
                })
                .collect();
            let rgba = Rgba {
                width: w,
                height: h,
                data: &px,
            };
            report(ctx, "OCR (PP-OCRv5 detector 1600×1200)", || {
                d.detect(&runner, &rgba).unwrap();
            });
            let mut rc = Recognizer::open(
                &rec.join("config.json"),
                &rec.join("model.safetensors"),
                &rec.join("preprocessor_config.json"),
                true,
            )
            .unwrap();
            let lines: Vec<specular_infer::ocr::lines::Img8> = (0..48)
                .map(|k| specular_infer::ocr::lines::Img8 {
                    width: 200 + 20 * k,
                    height: 48,
                    channels: 3,
                    data: (0..(200 + 20 * k) * 48 * 3)
                        .map(|i| (i * 31 % 255) as u8)
                        .collect(),
                })
                .collect();
            report(ctx, "OCR (PP-OCRv5 recogniser, 48 lines)", || {
                rc.read(&runner, &lines).unwrap();
            });
        }
        let tess = o.join("tessdata_best/heb.traineddata");
        if let (true, Some(ld)) = (tess.is_file(), dir("SPECULAR_TESS_LINES")) {
            let mut t = specular_infer::ocr::tess::Tesseract::open(&tess).unwrap();
            let lines: Vec<specular_infer::ocr::lines::Img8> = std::fs::read_dir(ld)
                .unwrap()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "pgm"))
                .map(|p| read_pgm(&p))
                .collect();
            report(ctx, "OCR (Tesseract heb, 60 lines)", || {
                t.read_gpu(ctx, &lines, 4).unwrap();
            });
        }
    }
    // Speech: Whisper encoders as the app chains them (base, large-v3-turbo).
    for (name, c) in [
        (
            "speech (Whisper base encoder)",
            whisper::Config {
                n_mels: 80,
                width: 512,
                heads: 8,
                encoder_layers: 6,
                decoder_layers: 6,
                audio_ctx: 1500,
                text_ctx: 448,
                vocab: 51_865,
                mlp: 2048,
            },
        ),
        (
            "speech (Whisper large-v3-turbo encoder)",
            whisper::Config {
                n_mels: 128,
                width: 1280,
                heads: 20,
                encoder_layers: 32,
                decoder_layers: 4,
                audio_ctx: 1500,
                text_ctx: 448,
                vocab: 51_866,
                mlp: 5120,
            },
        ),
    ] {
        let l = ctx.device().limits();
        let max_scores = l.max_storage_buffer_binding_size.min(l.max_buffer_size) / 2;
        let gs =
            whisper::encoder_graphs(&c, &mut rw(9), true, max_scores, whisper::GPU_MACS).unwrap();
        let mut ggs: Vec<GpuGraph> = gs.iter().map(|g| GpuGraph::new(ctx, g).unwrap()).collect();
        drop(gs);
        let mel = r.tensor(&[1, c.n_mels, c.frames()], 1.0);
        report(ctx, name, || {
            let mut h = ggs[0].run(&[Feed::Tensor(&mel)]).unwrap();
            for gg in &mut ggs[1..] {
                h = gg.run(&[Feed::Tensor(&h[0])]).unwrap();
            }
        });
    }
}

fn read_pgm(p: &std::path::Path) -> specular_infer::ocr::lines::Img8 {
    let b = std::fs::read(p).unwrap();
    let mut fields = Vec::new();
    let mut i = 0;
    while fields.len() < 4 {
        while b[i].is_ascii_whitespace() {
            i += 1;
        }
        let s = i;
        while !b[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(String::from_utf8_lossy(&b[s..i]).into_owned());
    }
    let (w, h): (usize, usize) = (fields[1].parse().unwrap(), fields[2].parse().unwrap());
    specular_infer::ocr::lines::Img8 {
        width: w,
        height: h,
        channels: 1,
        data: b[i + 1..i + 1 + w * h].to_vec(),
    }
}

/// A context on a device opened with the features inference wants (the app's has no
/// timestamp queries).
fn timestamp_context() -> Context {
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
    .unwrap();
    let a = adapter.limits();
    let mut limits = wgpu::Limits::default().using_resolution(a.clone());
    limits.max_storage_buffer_binding_size = a.max_storage_buffer_binding_size;
    limits.max_buffer_size = a.max_buffer_size;
    let features = adapter.features() & specular_infer::gpu::Features::wanted();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("submission report"),
        required_features: features,
        required_limits: limits,
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::MemoryUsage,
        trace: wgpu::Trace::Off,
    }))
    .unwrap();
    Context::new(&device, &queue)
}
