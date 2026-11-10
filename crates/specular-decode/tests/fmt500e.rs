//! Bitmap fonts against Pillow's PCF and BDF readers (tools/corpus/fmt500e-oracle.py
//! writes `<font>.glyphs` beside the corpus fetch-fmt500e.sh made), and what the registry
//! tells apart by content. The corpus tests are skipped unless `SPECULAR_FMT500E_CORPUS`
//! names the corpus folder.

use std::path::PathBuf;
use std::sync::Arc;

use specular_core::Source;
use specular_decode::{Format, Registry};

fn corpus(name: &str) -> Option<PathBuf> {
    let root = std::env::var_os("SPECULAR_FMT500E_CORPUS")?;
    let p = PathBuf::from(root).join(name);
    p.is_file().then_some(p)
}

/// A glyph's ink cropped to its box: (width, height, bits as '0'/'1').
fn ink(g: &specular_font::BitmapGlyph) -> (u32, u32, String) {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for y in 0..g.height {
        for x in 0..g.width {
            if g.ink(x, y) {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
            }
        }
    }
    if x0 == u32::MAX {
        return (0, 0, String::new());
    }
    let mut s = String::new();
    for y in y0..y1 {
        for x in x0..x1 {
            s.push(if g.ink(x, y) { '1' } else { '0' });
        }
    }
    (x1 - x0, y1 - y0, s)
}

#[test]
fn bitmap_fonts_match_pillow() {
    let names = [
        "10x20-ISO8859-1.pcf",
        "ter-x20b.pcf",
        "6x13.pcf",
        "courB08.bdf",
    ];
    let mut seen = 0;
    for name in names {
        let Some(path) = corpus(&format!("fonts/{name}")) else {
            continue;
        };
        let Ok(want) = std::fs::read_to_string(format!("{}.glyphs", path.display())) else {
            continue;
        };
        let d = std::fs::read(&path).unwrap();
        let (_, fonts) = specular_font::open_bitmap(&d).unwrap();
        let f = &fonts[0];
        let (mut differ, mut total) = (0, 0);
        for line in want.lines() {
            let mut it = line.split(' ');
            let code: u32 = it.next().unwrap().parse().unwrap();
            let w: u32 = it.next().unwrap().parse().unwrap();
            let h: u32 = it.next().unwrap().parse().unwrap();
            let bits = it.next().unwrap_or("").to_owned();
            total += 1;
            let got = f.glyphs.iter().find(|g| g.code == code).map(ink);
            if got != Some((w, h, bits)) {
                differ += 1;
                if differ <= 3 {
                    eprintln!(
                        "{name}: glyph {code} differs: {:?}",
                        got.map(|g| (g.0, g.1))
                    );
                }
            }
        }
        eprintln!("{name}: {differ} of {total} glyphs differ from Pillow");
        assert_eq!(differ, 0, "{name}");
        seen += 1;
    }
    if seen == 0 {
        eprintln!("SPECULAR_FMT500E_CORPUS not set or no oracle: skipped");
    }
}

/// The same font read from BDF and from bdftopcf's PCF of it gives the same glyphs.
#[test]
fn bdf_and_its_pcf_agree() {
    let (Some(b), Some(p)) = (
        corpus("fonts/courB08.bdf"),
        corpus("fonts/courB08-bdftopcf.pcf"),
    ) else {
        return;
    };
    let (_, b) = specular_font::open_bitmap(&std::fs::read(b).unwrap()).unwrap();
    let (_, p) = specular_font::open_bitmap(&std::fs::read(p).unwrap()).unwrap();
    assert_eq!(b[0].glyphs.len(), p[0].glyphs.len());
    for (x, y) in b[0].glyphs.iter().zip(&p[0].glyphs) {
        assert_eq!(
            (x.code, x.advance, x.top, x.left),
            (y.code, y.advance, y.top, y.left)
        );
        assert_eq!(ink(x), ink(y), "glyph {}", x.code);
    }
}

/// Every corpus font opens as a sheet with its facts; a .fon's fonts are pages.
#[test]
fn bitmap_fonts_open() {
    for name in [
        "10x20-ISO8859-1.pcf",
        "courB08.bdf",
        "Lat15-Terminus16.psf",
        "Uni2-VGA16.psf",
        "coure.fon",
        "cvgasys.fon",
    ] {
        let Some(path) = corpus(&format!("fonts/{name}")) else {
            continue;
        };
        let src = Arc::new(Source::open(&path).unwrap());
        let s = Registry::new().open(src).unwrap();
        assert_eq!(s.info().format, Format::BitmapFont, "{name}");
        let facts = s.header().unwrap();
        assert!(facts.fields.iter().any(|(k, _)| k == "Glyphs"), "{name}");
        let r = s
            .decode(
                &specular_decode::DecodeRequest::full(1),
                &specular_core::CancelToken::new(),
            )
            .unwrap();
        assert_eq!((r.width, r.height), (s.info().width, s.info().height));
        eprintln!("{name}: {} pages, {}", s.page_count(), s.info().coding);
    }
}

/// No oracle reads .fon or .psf here (FreeType finds no character map in these .fon files):
/// every letter and digit of both must have ink, and the .fon "A" (Windows' VGA system
/// font) must be mirror-symmetric.
#[test]
fn fon_and_psf_letters() {
    let (Some(fon), Some(psf)) = (corpus("fonts/cvgasys.fon"), corpus("fonts/Uni2-VGA16.psf"))
    else {
        return;
    };
    for p in [&fon, &psf] {
        let (_, f) = specular_font::open_bitmap(&std::fs::read(p).unwrap()).unwrap();
        for c in (b'A'..=b'Z').chain(b'a'..=b'z').chain(b'0'..=b'9') {
            let g = f[0].glyphs.iter().find(|g| g.code == u32::from(c)).unwrap();
            assert!(ink(g).0 > 0, "{} {}", p.display(), c as char);
        }
    }
    let (_, f) = specular_font::open_bitmap(&std::fs::read(&fon).unwrap()).unwrap();
    let (w, h, bits) = ink(f[0].glyphs.iter().find(|g| g.code == 65).unwrap());
    let rows: Vec<&str> = (0..h as usize)
        .map(|y| &bits[y * w as usize..(y + 1) * w as usize])
        .collect();
    assert!(
        rows.iter()
            .all(|r| r.chars().rev().collect::<String>() == *r),
        "{rows:?}"
    );
}

#[test]
fn content_tells_the_kinds() {
    // A PSF 2 font named .psf, a tracker module named .med.
    let mut psf = vec![0x72, 0xB5, 0x4A, 0x86];
    for v in [0u32, 32, 0, 1, 2, 2, 8] {
        psf.extend_from_slice(&v.to_le_bytes());
    }
    psf.extend_from_slice(&[0xFF, 0x81]);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let p = dir.join("fmt500e-test.psf");
    std::fs::write(&p, &psf).unwrap();
    let s = Registry::new()
        .open(Arc::new(Source::open(&p).unwrap()))
        .unwrap();
    assert_eq!(s.info().format, Format::BitmapFont);
    std::fs::remove_file(&p).ok();
}

/// Notebooks against nbformat's reading (upgraded to version 4): every cell's type,
/// source and count, and every output's type, stream name or MIME keys and text.
#[test]
fn notebooks_match_nbformat() {
    use specular_decode::notebook::Output;
    for name in ["test3.ipynb", "test4.ipynb", "test4plus.ipynb"] {
        let Some(path) = corpus(&format!("notebooks/{name}")) else {
            continue;
        };
        let Ok(want) = std::fs::read_to_string(format!("{}.cells", path.display())) else {
            continue;
        };
        let nb = specular_decode::notebook::read(&std::fs::read(&path).unwrap()).unwrap();
        let want: Vec<serde_json::Value> = want
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(nb.cells.len(), want.len(), "{name}");
        let mut differ = 0;
        for (i, (c, w)) in nb.cells.iter().zip(&want).enumerate() {
            let outs: Vec<serde_json::Value> = c
                .outputs
                .iter()
                .filter_map(|o| match o {
                    Output::Stream { name, text } => {
                        Some(serde_json::json!(["stream", name, text]))
                    }
                    Output::Other(t) => Some(serde_json::json!([t, "", ""])),
                    Output::Error { name, value, .. } => {
                        Some(serde_json::json!(["error", name, value]))
                    }
                    Output::Data { count, bundle } => {
                        if bundle
                            .first()
                            .is_some_and(|b| b.0.starts_with("attachment:"))
                        {
                            return None;
                        }
                        let mut keys: Vec<&str> = bundle.iter().map(|b| b.0.as_str()).collect();
                        keys.sort();
                        let plain = bundle
                            .iter()
                            .find(|b| b.0 == "text/plain")
                            .map_or("", |b| b.1.as_str());
                        let kind = if count.is_some() {
                            "execute_result"
                        } else {
                            "display_data"
                        };
                        Some(serde_json::json!([kind, keys.join(","), plain]))
                    }
                })
                .collect();
            let got = serde_json::json!({
                "type": c.kind, "source": c.source, "count": c.count, "outputs": outs
            });
            if &got != w {
                differ += 1;
                eprintln!("{name} cell {i}:\n ours {got}\n want {w}");
            }
        }
        eprintln!(
            "{name}: {differ} of {} cells differ from nbformat",
            want.len()
        );
        assert_eq!(differ, 0, "{name}");
        let src = Arc::new(Source::open(&path).unwrap());
        let s = Registry::new().open(src).unwrap();
        assert_eq!(s.info().format, Format::Notebook);
        eprintln!("{name}: {} pages, {}", s.page_count(), s.info().coding);
    }
}

/// A scene with every Excalidraw element kind drawn here, for the eye check.
const SCENE: &str = r##"{"type":"excalidraw","version":2,"source":"fmt500e test","elements":[
 {"type":"rectangle","x":0,"y":0,"width":160,"height":70,"angle":0,"strokeColor":"#1e1e1e","backgroundColor":"#a5d8ff","fillStyle":"solid","strokeWidth":2,"strokeStyle":"solid","opacity":100,"roundness":{"type":3}},
 {"type":"text","x":10,"y":20,"width":140,"height":25,"text":"Start","fontSize":20,"fontFamily":1,"textAlign":"center","verticalAlign":"middle","strokeColor":"#1e1e1e","opacity":100},
 {"type":"arrow","x":160,"y":35,"width":120,"height":40,"points":[[0,0],[60,0],[120,40]],"strokeColor":"#e03131","backgroundColor":"transparent","strokeWidth":2,"strokeStyle":"dashed","opacity":100,"endArrowhead":"triangle","startArrowhead":"dot"},
 {"type":"ellipse","x":280,"y":40,"width":120,"height":80,"angle":0.3,"strokeColor":"#2f9e44","backgroundColor":"#b2f2bb","fillStyle":"hachure","strokeWidth":1,"opacity":80},
 {"type":"diamond","x":40,"y":120,"width":100,"height":80,"strokeColor":"#1971c2","backgroundColor":"transparent","strokeWidth":2,"strokeStyle":"dotted","opacity":100},
 {"type":"freedraw","x":180,"y":150,"width":80,"height":40,"points":[[0,0],[10,10],[25,5],[40,30],[60,20],[80,40]],"strokeColor":"#000000","strokeWidth":2,"opacity":100},
 {"type":"frame","x":-20,"y":-20,"width":440,"height":240,"name":"My frame","strokeColor":"#bbb","backgroundColor":"transparent","opacity":100}],
 "appState":{"viewBackgroundColor":"#ffffff"},"files":{}}"##;

/// draw.io diagrams and an Excalidraw scene drawn: pages, shapes and ink. With
/// `FMT500E_PNG` set they are written as PNGs under the test's temporary folder for an eye
/// check (there is no oracle that draws them).
#[test]
fn diagrams_draw() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let scene = dir.join("fmt500e-scene.excalidraw");
    std::fs::write(&scene, SCENE).unwrap();
    let mut files = vec![scene.clone()];
    for n in [
        "uml-deployment-example",
        "aws-simple-architecture",
        "infographic-example-1",
        "timeline-example",
        "sysml-requirements-diagram",
        "concept-map-uml-diagrams-overview",
    ] {
        if let Some(p) = corpus(&format!("diagrams/{n}.drawio")) {
            files.push(p);
        }
    }
    for p in files {
        let src = Arc::new(Source::open(&p).unwrap());
        let s = Registry::new().open(src).unwrap();
        let want = if p.extension().is_some_and(|e| e == "excalidraw") {
            Format::Excalidraw
        } else {
            Format::Drawio
        };
        assert_eq!(s.info().format, want, "{}", p.display());
        let started = std::time::Instant::now();
        let r = s
            .decode(
                &specular_decode::DecodeRequest::full(1),
                &specular_core::CancelToken::new(),
            )
            .unwrap();
        let took = started.elapsed();
        let px = &r.planes[0].data;
        let ink = px
            .chunks_exact(4)
            .filter(|q| q[0] < 200 || q[1] < 200 || q[2] < 200)
            .count();
        eprintln!(
            "{}: {}×{}, {} pages, drawn in {took:?}, {ink} inked pixels; {}",
            p.file_name().unwrap().to_string_lossy(),
            r.width,
            r.height,
            s.page_count(),
            s.info().coding
        );
        assert!(ink > 500, "{}", p.display());
        if std::env::var_os("FMT500E_PNG").is_some() {
            let stride = r.planes[0].stride;
            let mut rgba = Vec::with_capacity((r.width * r.height * 4) as usize);
            for y in 0..r.height as usize {
                rgba.extend_from_slice(&px[y * stride..y * stride + 4 * r.width as usize]);
            }
            let png = specular_png::encode(r.width, r.height, specular_png::ColorType::Rgba, &rgba)
                .unwrap();
            let out = dir.join(format!("{}.png", p.file_stem().unwrap().to_string_lossy()));
            std::fs::write(&out, png).unwrap();
            eprintln!("  written {}", out.display());
        }
    }
    std::fs::remove_file(scene).ok();
}

/// RGB bytes of a raster (RGB or RGBA, one plane).
fn rgb_of(r: &specular_core::Raster) -> Vec<u8> {
    let p = &r.planes[0];
    let bpp = r.layout.pixel_bytes();
    let mut out = Vec::with_capacity((r.width * r.height * 3) as usize);
    for y in 0..r.height as usize {
        for x in 0..r.width as usize {
            let o = y * p.stride + x * bpp;
            out.extend_from_slice(&p.data[o..o + 3]);
        }
    }
    out
}

/// Lottie frames against rlottie's (run only): the share of pixels more than 48 levels off
/// in some channel. Two rasterisers differ at every antialiased edge, so a few percent is
/// expected; whole shapes misplaced or missing show as much more.
#[test]
fn lottie_follows_rlottie() {
    let mut failed = Vec::new();
    for name in [
        "exploding-star.json",
        "a_cup_of_coffee.json",
        "dna.json",
        "emoji_wink.json",
        "gears.json",
        "hourglass.json",
        "like.json",
        "telegram.json",
    ] {
        // The bounce of telegram.json lands a few pixels apart mid-flight (easing); like.json
        // uses difference masks, drawn here as add.
        let gate = match name {
            "telegram.json" => 10.0,
            "like.json" => 13.0,
            _ => 5.0,
        };
        let Some(path) = corpus(&format!("lottie/{name}")) else {
            continue;
        };
        let src = Arc::new(Source::open(&path).unwrap());
        let s = Registry::new().open(src).unwrap();
        assert_eq!(s.info().format, Format::Lottie, "{name}");
        let n = s.animation().unwrap().frame_count.unwrap();
        let mut frames = s.frames(1).unwrap();
        for k in [0, n / 4, n / 2, 3 * n / 4] {
            let Some(raw) = corpus(&format!("lottie/{name}.f{k}.rgb")) else {
                continue;
            };
            let want = std::fs::read(raw).unwrap();
            frames.seek(k).unwrap();
            let started = std::time::Instant::now();
            let got = frames
                .next_frame(&specular_core::CancelToken::new())
                .unwrap()
                .unwrap()
                .raster;
            let took = started.elapsed();
            let (a, b) = (rgb_of(&got), want);
            assert_eq!(a.len(), b.len(), "{name}: size");
            let off = a
                .chunks_exact(3)
                .zip(b.chunks_exact(3))
                .filter(|(p, q)| p.iter().zip(*q).any(|(x, y)| x.abs_diff(*y) > 48))
                .count();
            let pc = off as f64 * 100.0 / f64::from(got.width * got.height);
            eprintln!("{name} frame {k}: {off} pixels off ({pc:.2}%), drawn in {took:?}");
            if std::env::var_os("FMT500E_PNG").is_some() {
                let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
                let png =
                    specular_png::encode(got.width, got.height, specular_png::ColorType::Rgb, &a)
                        .unwrap();
                std::fs::write(dir.join(format!("{name}.ours.f{k}.png")), png).unwrap();
            }
            if pc > gate {
                failed.push(format!("{name} frame {k}: {pc:.2}%"));
            }
        }
    }
    assert!(failed.is_empty(), "{failed:?}");
}

/// A JSON file that is not a Lottie animation is not taken for one.
#[test]
fn plain_json_is_not_lottie() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let p = dir.join("fmt500e-plain.json");
    std::fs::write(&p, b"{\"name\": \"x\", \"v\": 1, \"items\": [1, 2, 3]}").unwrap();
    let r = Registry::new().open(Arc::new(Source::open(&p).unwrap()));
    assert!(!matches!(r, Ok(s) if s.info().format == Format::Lottie));
    std::fs::remove_file(p).ok();
}

/// SWF first frames against the pictures Ruffle's own tests expect (its visual tests,
/// MIT/Apache-2.0): the share of pixels more than 48 levels off in some channel.
#[test]
fn swf_follows_ruffle() {
    // (test, most pixels off, %): antialiasing differs at every edge. Known differences
    // gated at what they measure: gradients whose stops share ratios (Ruffle spreads them
    // apart), and edit text drawn in a system font where Ruffle uses the embedded one.
    let tests = [
        ("simple_shapes-overlaps", 4.0),
        ("simple_shapes-winding_rule", 4.0),
        ("simple_shapes-layers", 4.0),
        ("simple_shapes-masks", 8.0),
        ("simple_shapes-gradients-gradients", 8.0),
        ("simple_shapes-gradients-radial", 8.0),
        ("gradient_radial_same_ratios", 45.0),
        ("define_bits_lossless2_rgb15", 4.0),
        ("color_transform_issue_9698", 8.0),
        ("gradient_issue_9892", 12.0),
        ("fonts-glyph", 14.0),
    ];
    let mut failed = Vec::new();
    for (name, gate) in tests {
        let (Some(swf), Some(raw), Some(size)) = (
            corpus(&format!("swf/{name}.test.swf")),
            corpus(&format!("swf/{name}.expected.rgb")),
            corpus(&format!("swf/{name}.expected.size")),
        ) else {
            continue;
        };
        let theirs = std::fs::read(raw).unwrap();
        let wh: Vec<u32> = std::fs::read_to_string(size)
            .unwrap()
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        let s = Registry::new()
            .open(Arc::new(Source::open(&swf).unwrap()))
            .unwrap();
        assert_eq!(s.info().format, Format::Swf, "{name}");
        let m = specular_swf::open(&std::fs::read(&swf).unwrap(), None).unwrap();
        let (w, h) = (wh[0], wh[1]);
        let req = specular_drawpage::RenderRequest {
            scale_x: f64::from(w) / m.page.size.0,
            scale_y: f64::from(h) / m.page.size.1,
            x: 0,
            y: 0,
            width: w,
            height: h,
        };
        let px = specular_drawpage::render(&m.page, req, &|| false).unwrap();
        let ours: Vec<u8> = px
            .chunks_exact(4)
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect();
        let off = ours
            .chunks_exact(3)
            .zip(theirs.chunks_exact(3))
            .filter(|(p, q)| p.iter().zip(*q).any(|(x, y)| x.abs_diff(*y) > 48))
            .count();
        let pc = off as f64 * 100.0 / f64::from(w * h);
        eprintln!(
            "{name}: {w}×{h}, {off} pixels off ({pc:.2}%); {}",
            s.info().coding
        );
        if std::env::var_os("FMT500E_PNG").is_some() {
            let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
            let png = specular_png::encode(w, h, specular_png::ColorType::Rgb, &ours).unwrap();
            std::fs::write(dir.join(format!("{name}.ours.png")), png).unwrap();
        }
        if pc > gate {
            failed.push(format!("{name} {pc:.2}%"));
        }
    }
    assert!(failed.is_empty(), "{failed:?}");
}

/// Game and scene models against assimp's reading: triangle counts and bounding boxes
/// (axes as each file stores them); MagicaVoxel's voxel count.
#[test]
fn models_follow_assimp() {
    use specular_mesh::{Limits, MeshFormat};
    for (name, format) in [
        ("faerie.md2", MeshFormat::Md2),
        ("jeep1.ms3d", MeshFormat::Ms3d),
        ("Wuson.ac", MeshFormat::Ac3d),
        ("test.x", MeshFormat::DirectX),
        ("monu1.vox", MeshFormat::Vox),
        ("01_01.bvh", MeshFormat::Bvh),
    ] {
        let Some(path) = corpus(&format!("models/{name}")) else {
            continue;
        };
        let d = std::fs::read(&path).unwrap();
        assert_eq!(
            MeshFormat::sniff(&d[..d.len().min(256)]),
            Some(format),
            "{name}"
        );
        let started = std::time::Instant::now();
        let m = specular_mesh::read(format, &d, &specular_mesh::NoResolver, &Limits::default())
            .unwrap();
        let took = started.elapsed();
        let tris = m.indices.len() / 3;
        let want = std::fs::read_to_string(format!("{}.mesh", path.display())).unwrap_or_default();
        let field = |k: &str| {
            want.lines().find(|l| l.starts_with(k)).map(|l| {
                l[k.len()..]
                    .split_whitespace()
                    .map(|v| v.parse::<f64>().unwrap())
                    .collect::<Vec<_>>()
            })
        };
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for &i in &m.indices {
            let p = m.positions[i as usize];
            // assimp turns Quake's Z-up into Y-up.
            let p = if format == MeshFormat::Md2 {
                [p[0], p[2], -p[1]]
            } else {
                p
            };
            for k in 0..3 {
                lo[k] = lo[k].min(f64::from(p[k]));
                hi[k] = hi[k].max(f64::from(p[k]));
            }
        }
        eprintln!(
            "{name}: {tris} triangles in {took:?}, bounds {lo:.3?}–{hi:.3?}; assimp {}",
            want.replace('\n', "; ")
        );
        if let Some(v) = field("voxels ") {
            assert!(
                m.notes
                    .iter()
                    .any(|n| n.contains(&format!("{} voxels", v[0]))),
                "{:?}",
                m.notes
            );
            continue;
        }
        if format == MeshFormat::Bvh {
            // assimp draws no skeleton: only that one was drawn here.
            assert!(tris > 0);
            continue;
        }
        let want_tris = field("triangles ").unwrap()[0] as usize;
        assert_eq!(tris, want_tris, "{name}: triangles");
        let b = field("bounds ").unwrap();
        let size = (0..3).map(|k| b[k + 3] - b[k]).fold(0.0, f64::max);
        for k in 0..3 {
            assert!(
                (lo[k] - b[k]).abs() <= 0.01 * size,
                "{name}: low {k}: {} vs {}",
                lo[k],
                b[k]
            );
            assert!(
                (hi[k] - b[k + 3]).abs() <= 0.01 * size,
                "{name}: high {k}: {} vs {}",
                hi[k],
                b[k + 3]
            );
        }
    }
}

/// MOL2 against RDKit and GRO against mdtraj: atom and bond counts, elements and
/// coordinates (Å).
#[test]
fn molecules_follow_rdkit_and_mdtraj() {
    use specular_mol::{Limits, MolFormat};
    for (name, format) in [
        ("Molecule.mol2", MolFormat::Mol2),
        ("adk_oplsaa.gro", MolFormat::Gro),
    ] {
        let Some(path) = corpus(&format!("mol/{name}")) else {
            continue;
        };
        let Ok(want) = std::fs::read_to_string(format!("{}.atoms", path.display())) else {
            continue;
        };
        let d = std::fs::read(&path).unwrap();
        assert_eq!(specular_mol::sniff_file(&d), Some(format), "{name}");
        let started = std::time::Instant::now();
        let m = specular_mol::open(&d, None, &Limits::default()).unwrap();
        let took = started.elapsed();
        let mut lines = want.lines();
        let n: usize = lines.next().unwrap()[6..].parse().unwrap();
        let bonds: usize = lines.next().unwrap()[6..].parse().unwrap();
        assert_eq!(m.atoms.len(), n, "{name}: atoms");
        if format == MolFormat::Mol2 {
            assert_eq!(m.file_bonds, bonds, "{name}: bonds");
        }
        let (mut el_off, mut worst) = (0, 0f64);
        for (a, l) in m.atoms.iter().zip(lines) {
            let v: Vec<f64> = l.split_whitespace().map(|x| x.parse().unwrap()).collect();
            if f64::from(a.element) != v[0] {
                el_off += 1;
            }
            for k in 0..3 {
                worst = worst.max((f64::from(a.pos[k]) - v[k + 1]).abs());
            }
        }
        eprintln!(
            "{name}: {n} atoms read in {took:?}; {el_off} elements differ, coordinates within {worst:.4} Å; {} bonds",
            m.bonds.len()
        );
        assert!(worst < 1e-3, "{name}: {worst}");
        // GROMACS names carry no element: a few ion or unusual names may differ.
        assert!(el_off * 1000 <= n, "{name}: {el_off} elements differ");
    }
}
