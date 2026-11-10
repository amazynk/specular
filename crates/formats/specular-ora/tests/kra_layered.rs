//! Krita composites against Krita's own merged image over the layered-editor corpus
//! (`tools/corpus/fetch-layered.sh`: `tools/corpus/igapA/kra`, or `$SPECULAR_KRA_LAYERED`):
//! real documents with vector, clone, fill, filter and file layers, filter masks and blend
//! modes, each saved by Krita with `mergedimage.png`, the oracle.
//!
//! Every file with a merged image in an 8- or 16-bit RGB or grey model is composited (vector
//! layers through resvg, as the viewer draws SVG; file layers from beside the document) and
//! compared premultiplied in 8-bit. A file matches when at most 0.5 % of its pixels are off by
//! more than 2 levels in a channel, and comes close at a PSNR of 30 dB or more (vector
//! shapes: Qt's and tiny-skia's anti-aliased edges differ by a level or two); both pass. Prints one line per file and the totals by feature; files
//! that draw everything (no notes) must pass, but for the few listed in `KNOWN`.
//! Skipped, with a message, when the corpus is absent.
//!
//! `KRA_ONLY=<substring>` keeps matching files; `KRA_VERBOSE=1` prints the first mismatch.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use resvg::{tiny_skia, usvg};
use specular_ora::{Document, KraBlend, KraHost, KraKind, KraOutput};

fn corpus() -> PathBuf {
    std::env::var_os("SPECULAR_KRA_LAYERED").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/igapA/kra"),
        PathBuf::from,
    )
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("kra")) {
            out.push(p);
        }
    }
}

fn fonts() -> &'static usvg::Options<'static> {
    static O: OnceLock<usvg::Options<'static>> = OnceLock::new();
    O.get_or_init(|| {
        resvg::set_raster_decoder(png_raster);
        let mut o = usvg::Options::default();
        o.fontdb_mut().load_system_fonts();
        o
    })
}

/// PNG images inside the SVG (Krita's pattern fills), as the viewer decodes them.
fn png_raster(kind: &usvg::ImageKind) -> Option<std::sync::Arc<tiny_skia::Pixmap>> {
    let usvg::ImageKind::PNG(d) = kind else {
        return None;
    };
    let img = specular_png::decode(
        d,
        &specular_png::DecodeOptions {
            output: specular_png::Output::Rgba8,
            ..Default::default()
        },
    )
    .ok()?;
    let mut px = img.planes[0].data.clone();
    for p in px.chunks_exact_mut(4) {
        let a = u32::from(p[3]);
        for c in &mut p[..3] {
            *c = ((u32::from(*c) * a + 127) / 255) as u8;
        }
    }
    let size = tiny_skia::IntSize::from_wh(img.width, img.height)?;
    tiny_skia::Pixmap::from_vec(px, size).map(std::sync::Arc::new)
}

/// The test's host: SVG through resvg, file layers (PNG) from the document's folder.
struct Host {
    dir: PathBuf,
}

impl KraHost for Host {
    fn svg(&self, svg: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
        let tree = usvg::Tree::from_data(svg, fonts()).ok()?;
        let mut pm = tiny_skia::Pixmap::new(w, h)?;
        let s = tree.size();
        let t = tiny_skia::Transform::from_scale(w as f32 / s.width(), h as f32 / s.height());
        resvg::render(&tree, t, &mut pm.as_mut());
        Some(pm.take())
    }

    fn file(&self, source: &str) -> Result<(u32, u32, Vec<u8>), String> {
        let p = self.dir.join(source);
        let d = std::fs::read(&p).map_err(|_| format!("{source} not found"))?;
        let img = specular_png::decode(
            &d,
            &specular_png::DecodeOptions {
                output: specular_png::Output::Rgba8,
                ..Default::default()
            },
        )
        .map_err(|e| e.to_string())?;
        Ok((img.width, img.height, img.planes[0].data.clone()))
    }

    fn locate(&self, source: &str) -> Result<(), String> {
        if self.dir.join(source).is_file() {
            Ok(())
        } else {
            Err(format!("{source} not found beside the document"))
        }
    }
}

/// The features of a document this lane covers, for the totals.
fn features(doc: &Document) -> Vec<String> {
    let mut f = Vec::new();
    for n in &doc.kra_nodes {
        if !n.layer.visible {
            continue;
        }
        let k = match &n.kind {
            KraKind::Filter(s) => format!("filter:{}", s.name),
            KraKind::Fill(s) => format!("fill:{}", s.name),
            KraKind::Vector { .. } => "vector".into(),
            KraKind::Clone { .. } => "clone".into(),
            KraKind::File { .. } => "file".into(),
            _ => String::new(),
        };
        if !k.is_empty() {
            f.push(k);
        }
        for m in &n.layer.masks {
            if let Some(s) = &m.filter {
                f.push(format!("filtermask:{}", s.name));
            }
        }
        if n.layer.blend != "normal" {
            let known = KraBlend::from_id(&n.layer.blend).is_some();
            f.push(format!(
                "op:{}{}",
                n.layer.blend,
                if known { "" } else { "?" }
            ));
        }
    }
    f.sort();
    f.dedup();
    f
}

/// Files that draw everything yet do not match, and why.
const KNOWN: &[(&str, &str)] = &[
    (
        "357987_96656",
        "Krita drew a pattern fill layer that is no longer in maindoc.xml (the reported bug)",
    ),
    (
        "375879_",
        "Krita 3's multiply kept the backdrop's alpha (the reported bug)",
    ),
    (
        "399363_",
        "Krita's merged image has a damaged strip along the bottom (the reported bug)",
    ),
    (
        "400427_115961",
        "Krita's merged image predates the blur and vector layers (the reported bug)",
    ),
    (
        "423515_",
        "the vector shapes Krita drew were misaligned (the reported bug)",
    ),
    (
        "439759_",
        "Krita drew the vector pattern fill at another scale (the reported bug)",
    ),
    (
        "439725_",
        "Krita drew the vector pattern fill at another scale (the reported bug)",
    ),
];

#[test]
fn composites_match_kritas_merged_image() {
    let mut files = Vec::new();
    walk(&corpus(), &mut files);
    files.sort();
    if files.is_empty() {
        eprintln!(
            "no corpus at {} (tools/corpus/fetch-layered.sh): skipped",
            corpus().display()
        );
        return;
    }
    let only = std::env::var("KRA_ONLY").ok();
    let verbose = std::env::var_os("KRA_VERBOSE").is_some();
    // feature -> (files, passed, files without notes, of those passed)
    let mut by: BTreeMap<String, [usize; 4]> = BTreeMap::new();
    let (mut total, mut passed, mut clean, mut clean_passed) = (0, 0, 0, 0);
    let mut failures = Vec::new();
    for f in files {
        let name = f.file_name().unwrap().to_string_lossy().into_owned();
        if only.as_ref().is_some_and(|o| !name.contains(o.as_str())) {
            continue;
        }
        let data = std::fs::read(&f).unwrap();
        let Ok(doc) = Document::open(&data) else {
            println!("{name}\tnot a Krita document");
            continue;
        };
        let Some(m) = doc.merged() else { continue };
        if !doc.kra_can_composite() {
            println!("{name}\tskip\tnothing to composite");
            continue;
        }
        let host = Host {
            dir: f.parent().unwrap().to_path_buf(),
        };
        let img = match doc.kra_composite_with(&data, Some(&host), &|| false) {
            Ok(i) => i,
            Err(e) => {
                println!("{name}\terror\t{e}");
                continue;
            }
        };
        if img.format == KraOutput::Rgba16F || img.format == KraOutput::GrayAlpha16 {
            println!("{name}\tskip\t{:?} not compared", img.format);
            continue;
        }
        let wide = img.format == KraOutput::Rgba16;
        let png = doc.read(&data, m).unwrap();
        if wide && png.get(24) != Some(&16) {
            println!("{name}\tskip\tmerged image converted to 8-bit");
            continue;
        }
        let Ok(merged) = specular_png::decode(
            &png,
            &specular_png::DecodeOptions {
                output: if wide {
                    specular_png::Output::Rgba16
                } else {
                    specular_png::Output::Rgba8
                },
                ..Default::default()
            },
        ) else {
            println!("{name}\tskip\tmerged image unreadable");
            continue;
        };
        let px = |d: &[u8], i: usize| -> [f32; 4] {
            let v: [f32; 4] = if wide {
                std::array::from_fn(|c| {
                    let b = &d[(i * 4 + c) * 2..][..2];
                    f32::from(u16::from_ne_bytes([b[0], b[1]])) / 257.0
                })
            } else {
                std::array::from_fn(|c| f32::from(d[i * 4 + c]))
            };
            let a = v[3] / 255.0;
            [v[0] * a, v[1] * a, v[2] * a, v[3]]
        };
        let (w, h) = (img.width as usize, img.height as usize);
        let (mw, mh) = (merged.width as usize, merged.height as usize);
        let (mut off, mut n, mut se) = (0usize, 0usize, 0f64);
        let mut first = None;
        for y in 0..h.min(mh) {
            for x in 0..w.min(mw) {
                let a = px(&img.data, y * w + x);
                let b = px(&merged.planes[0].data, y * mw + x);
                let mut d = 0f32;
                for c in 0..4 {
                    let e = (a[c] - b[c]).abs();
                    d = d.max(e);
                    se += f64::from(e * e);
                }
                if d > 2.5 {
                    off += 1;
                    if first.is_none() {
                        first = Some((x, y, a, b));
                    }
                }
                n += 1;
            }
        }
        let mse = se / (n as f64 * 4.0);
        let psnr = if mse > 0.0 {
            10.0 * (255.0f64 * 255.0 / mse).log10()
        } else {
            f64::INFINITY
        };
        let notes = doc.kra_notes(&data, None, Some(&host));
        let feats = features(&doc);
        let exact = off as f64 <= n as f64 * 0.005;
        let ok = exact || psnr >= 30.0;
        println!(
            "{name}\t{}\t{off} of {n} px off\tPSNR {psnr:.1} dB\t{}\tnotes: {}",
            if exact {
                "pass"
            } else if ok {
                "close"
            } else {
                "FAIL"
            },
            feats.join(","),
            notes.join("; ")
        );
        if verbose && let Some(f) = first {
            println!("  first at {f:?} (composite, merged; premultiplied)");
        }
        // `KRA_DUMP=<dir>`: the composite as raw straight RGBA8, `<name>.<w>x<h>.rgba`.
        if let Some(dir) = std::env::var_os("KRA_DUMP") {
            let raw: Vec<u8> = (0..w * h)
                .flat_map(|i| {
                    let p = px(&img.data, i);
                    let a = p[3];
                    let s = |c: f32| {
                        if a > 0.0 {
                            (c * 255.0 / a).round() as u8
                        } else {
                            0
                        }
                    };
                    [s(p[0]), s(p[1]), s(p[2]), a.round() as u8]
                })
                .collect();
            std::fs::write(Path::new(&dir).join(format!("{name}.{w}x{h}.rgba")), raw).unwrap();
        }
        total += 1;
        passed += usize::from(ok);
        if notes.is_empty() {
            clean += 1;
            clean_passed += usize::from(ok);
            if !ok && !KNOWN.iter().any(|(k, _)| name.contains(k)) {
                failures.push(name.clone());
            }
        }
        for k in feats {
            let e = by.entry(k).or_default();
            e[0] += 1;
            e[1] += usize::from(ok);
            if notes.is_empty() {
                e[2] += 1;
                e[3] += usize::from(ok);
            }
        }
    }
    println!("\nfeature\tfiles\tpass\tdrawn fully\tof those pass");
    for (k, v) in &by {
        println!("{k}\t{}\t{}\t{}\t{}", v[0], v[1], v[2], v[3]);
    }
    println!(
        "TOTAL\t{total} compared, {passed} pass; {clean} drawn fully, {clean_passed} of those pass"
    );
    assert!(
        failures.is_empty(),
        "fully drawn files that do not match: {failures:?}"
    );
}

/// `KRA_SVG_FILE=<kra> KRA_SVG_MEMBER=<member> KRA_SVG_OUT=<raw>`: one vector layer as the
/// test host draws it (raw premultiplied RGBA8 at the canvas size), for debugging.
#[test]
#[ignore = "debugging aid"]
fn draw_one_vector_layer() {
    let (Some(f), Some(m), Some(o)) = (
        std::env::var_os("KRA_SVG_FILE"),
        std::env::var("KRA_SVG_MEMBER").ok(),
        std::env::var_os("KRA_SVG_OUT"),
    ) else {
        return;
    };
    let data = std::fs::read(f).unwrap();
    let doc = Document::open(&data).unwrap();
    let (w, h) = doc.canvas.unwrap();
    let svg = doc.read(&data, doc.zip.find(&m).unwrap()).unwrap();
    let fixed = specular_ora::kra_svg::fixup(&svg);
    println!("fixup: {}", fixed.is_some());
    let svg = fixed.unwrap_or(svg.to_vec());
    let host = Host {
        dir: PathBuf::new(),
    };
    let px = host.svg(&svg, w, h).unwrap();
    std::fs::write(o, px).unwrap();
    println!("{w}x{h}");
}
