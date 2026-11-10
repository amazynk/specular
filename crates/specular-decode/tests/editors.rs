//! Editor and legacy formats through the registry: Paint.NET, CorelDRAW, BPG (and PSP,
//! FLIF). Committed fixtures run always; the corpus parts (tools/corpus/fetch-editors*.sh)
//! skip when it is absent. Decoding itself is tested against each format's oracle in its
//! own crate.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::{CancelToken, PixelLayout, Source};
use specular_decode::{DecodeRequest, Format, PageLabel, Registry, Session};

#[path = "common/trim.rs"]
mod trim;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn open(path: &Path) -> Arc<dyn Session> {
    Registry::new()
        .open_path(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn rgba(s: &dyn Session) -> (u32, u32, Vec<u8>, PixelLayout) {
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    (r.width, r.height, r.planes[0].data.clone(), r.layout)
}

fn png_rgba(path: &Path) -> (u32, u32, Vec<u8>) {
    let d = std::fs::read(path).unwrap();
    let img = specular_png::decode(
        &d,
        &specular_png::DecodeOptions {
            output: specular_png::Output::Rgba8,
            ..Default::default()
        },
    )
    .unwrap();
    (img.width, img.height, img.to_rgba8())
}

fn corpus(sub: &str) -> Option<PathBuf> {
    let p = std::env::var_os("SPECULAR_EDITORS_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| here().join("../../tools/corpus/editors"))
        .join(sub);
    p.is_dir().then_some(p)
}

fn files(dir: &Path, ext: &[&str]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| ext.iter().any(|x| e.eq_ignore_ascii_case(x)))
            {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn paint_net_document_pages_and_preview() {
    let path = here().join("tests/editors/pypdn-untitled3.pdn");
    let s = open(&path);
    let i = s.info();
    assert_eq!((i.format, i.width, i.height), (Format::Pdn, 800, 600));
    assert_eq!(
        i.coding, "Paint.NET 4.21, 2 layers: Layer 2 (Additive, 63 %), Background",
        "{}",
        i.coding
    );
    assert_eq!(s.colour_model(), Some("RGB"));
    // As Paint.NET flattened it.
    let (w, h, px, layout) = rgba(s.as_ref());
    assert_eq!(layout, PixelLayout::Rgba);
    let (rw, rh, want) = png_rgba(&here().join("tests/editors/pypdn-untitled3.png"));
    assert_eq!((w, h), (rw, rh));
    assert_eq!(px, want);
    // The header thumbnail is the preview.
    let p = s.decode_preview().unwrap().unwrap();
    assert_eq!((p.width, p.height), (256, 192));
    // Pages: the composite, then the layers top-most first.
    assert_eq!(s.page_count(), 3);
    assert_eq!(s.page_label(), Some(PageLabel::Merged));
    let top = s.open_page(1).unwrap();
    assert!(matches!(
        top.page_label(),
        Some(PageLabel::DocumentLayer { ref name, index: 1, hidden: false }) if &**name == "Layer 2"
    ));
    assert!(top.decode_preview().is_none());
    let bg = s.open_page(2).unwrap();
    let (_, _, bpx, _) = rgba(bg.as_ref());
    assert_eq!(&bpx[3..4], &[255]);
    assert!(s.open_page(3).is_err());
    trim::assert_trims(s.as_ref(), &DecodeRequest::full(2));
}

#[test]
fn damaged_paint_net_documents_fail_cleanly() {
    let good = std::fs::read(here().join("tests/editors/pypdn-untitled3.pdn")).unwrap();
    let reg = Registry::new();
    for cut in [10, 200, 3000, good.len() - 500, good.len() - 1] {
        let src = Arc::new(Source::from_vec("cut.pdn", good[..cut].to_vec()));
        if let Ok(s) = reg.open(src) {
            let _ = s.decode(&DecodeRequest::full(1), &CancelToken::new());
        }
    }
}

#[test]
fn bpg_fixtures() {
    let dir = here().join("../formats/specular-bpg/tests/data");
    let s = open(&dir.join("anim-alpha-delays.bpg"));
    assert_eq!(s.info().format, Format::Bpg);
    assert!(s.info().has_alpha);
    let a = s.animation().unwrap();
    assert_eq!(a.frame_count, Some(6));
    let mut f = s.frames(2).unwrap();
    let first = f.next_frame(&CancelToken::new()).unwrap().unwrap();
    assert_eq!((first.raster.width, first.raster.height), (80, 50));
    assert_eq!(first.delay, std::time::Duration::from_millis(80));
    let mut n = 1;
    while let Some(fr) = f.next_frame(&CancelToken::new()) {
        fr.unwrap();
        n += 1;
    }
    assert_eq!(n, 6);
    let g = open(&dir.join("grey-10.bpg"));
    assert_eq!(g.colour_model(), Some("Greyscale"));
    assert_eq!(rgba(g.as_ref()).3, PixelLayout::Gray16);
    let y = open(&dir.join("logo-ycgco-420.bpg"));
    assert_eq!(y.colour_model(), Some("YCgCo"));
    assert_eq!(y.chroma_subsampling(), Some("4:2:0"));
    assert!(
        y.info().coding.starts_with("BPG 8-bit YCgCo 4:2:0"),
        "{}",
        y.info().coding
    );
    let m = open(&dir.join("meta-exif.bpg"));
    assert!(
        m.embedded_exif()
            .is_some_and(|e| e.starts_with(b"II*\0") || e.starts_with(b"MM\0*"))
    );
}

#[test]
fn corpus_paint_net_opens_and_composites() {
    let Some(dir) = corpus("pdn") else {
        eprintln!("PDN corpus absent: skipped");
        return;
    };
    let mut n = 0;
    for f in files(&dir, &["pdn"]) {
        let s = open(&f);
        let t0 = std::time::Instant::now();
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert!(!r.truncated, "{}", f.display());
        for p in 1..s.page_count() {
            s.open_page(p).unwrap();
        }
        eprintln!(
            "{}: {} ({:.1} ms)",
            f.file_name().unwrap().to_string_lossy(),
            s.info().coding,
            t0.elapsed().as_secs_f64() * 1e3
        );
        n += 1;
    }
    eprintln!("Paint.NET: {n} documents opened and composited");
}

#[test]
fn corpus_coreldraw_previews_match_pillow() {
    let Some(dir) = corpus("cdr") else {
        eprintln!("CDR corpus absent: skipped");
        return;
    };
    let reg = Registry::new();
    let (mut equal, mut differ, mut refused) = (0, 0, 0);
    for f in files(&dir, &["cdr"]) {
        let s = match reg.open_path(&f) {
            Ok(s) => s,
            Err(e) => {
                refused += 1;
                eprintln!("{}: {e}", f.file_name().unwrap().to_string_lossy());
                continue;
            }
        };
        assert_eq!(s.info().format, Format::Cdr);
        assert!(s.info().coding.contains("embedded preview only"));
        let reference = PathBuf::from(format!("{}.preview.png", f.display()));
        if !reference.exists() {
            continue;
        }
        let (rw, rh, want) = png_rgba(&reference);
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        let mut got = Vec::with_capacity((r.width * r.height * 4) as usize);
        let p = &r.planes[0];
        for y in 0..r.height as usize {
            let row = &p.data[y * p.stride..];
            for x in 0..r.width as usize {
                match r.layout {
                    PixelLayout::Rgba => got.extend_from_slice(&row[x * 4..x * 4 + 4]),
                    PixelLayout::Gray => got.extend_from_slice(&[row[x], row[x], row[x], 255]),
                    PixelLayout::Rgb => {
                        let c = |i: usize| r.planes[i].data[y * r.planes[i].stride + x];
                        got.extend_from_slice(&[c(0), c(1), c(2), 255]);
                    }
                    l => panic!("{}: layout {l:?}", f.display()),
                }
            }
        }
        if (r.width, r.height) == (rw, rh) && got == want {
            equal += 1;
        } else {
            differ += 1;
            eprintln!("{}: differs from Pillow", f.display());
        }
    }
    eprintln!("CorelDRAW previews: {equal} equal to Pillow, {differ} differ, {refused} refused");
    assert_eq!(differ, 0);
}

#[test]
fn paintshop_pro_fixtures() {
    let dir = here().join("../formats/specular-psp/tests/data");
    for f in files(&dir, &["psp", "tub"]) {
        let s = open(&f);
        assert_eq!(s.info().format, Format::Psp, "{}", f.display());
        assert!(
            s.info().coding.starts_with("PaintShop Pro file format"),
            "{}",
            s.info().coding
        );
        let (w, h, _, _) = rgba(s.as_ref());
        assert_eq!((w, h), (s.info().width, s.info().height));
        for p in 1..s.page_count() {
            let l = s.open_page(p).unwrap();
            assert!(matches!(
                l.page_label(),
                Some(PageLabel::DocumentLayer { .. })
            ));
            rgba(l.as_ref());
        }
        if let Some(t) = s.decode_preview() {
            t.unwrap();
        }
    }
    let layered = open(&dir.join("layers3-rle-v3.psp"));
    assert!(layered.page_count() > 1);
    assert_eq!(layered.page_label(), Some(PageLabel::Merged));
}

#[test]
fn corpus_paintshop_pro_opens() {
    let Some(dir) = corpus("psp") else {
        eprintln!("PSP corpus absent: skipped");
        return;
    };
    let reg = Registry::new();
    let (mut ok, mut refused) = (0, 0);
    for f in files(&dir, &["psp", "pspimage", "tub", "psptube"]) {
        match reg.open_path(&f) {
            Ok(s) => {
                let t0 = std::time::Instant::now();
                s.decode(&DecodeRequest::full(1), &CancelToken::new())
                    .unwrap_or_else(|e| panic!("{}: {e}", f.display()));
                eprintln!(
                    "{}: {} ({:.1} ms)",
                    f.file_name().unwrap().to_string_lossy(),
                    s.info().coding,
                    t0.elapsed().as_secs_f64() * 1e3
                );
                ok += 1;
            }
            Err(e) => {
                refused += 1;
                eprintln!("{}: {e}", f.display());
            }
        }
    }
    eprintln!("PaintShop Pro: {ok} opened, {refused} refused");
}

#[test]
fn flif_fixtures() {
    let dir = here().join("../formats/specular-flif/tests/data");
    for f in files(&dir, &["flif"]) {
        let s = open(&f);
        assert_eq!(s.info().format, Format::Flif, "{}", f.display());
        let (w, h, _, _) = rgba(s.as_ref());
        assert_eq!((w, h), (s.info().width, s.info().height));
        // Every level has the size of a box-filtered level, interlaced or not.
        for scale in [2, 4, 8] {
            let r = s
                .decode(&DecodeRequest::full(scale), &CancelToken::new())
                .unwrap();
            assert_eq!(
                (r.width, r.height),
                (w.div_ceil(scale), h.div_ceil(scale)),
                "{} at 1/{scale}",
                f.display()
            );
        }
    }
    let a = open(&dir.join("anim-rgb-I.flif"));
    let n = a.animation().unwrap().frame_count.unwrap();
    assert!(n > 1);
    let mut fr = a.frames(1).unwrap();
    let mut count = 0;
    while let Some(f) = fr.next_frame(&CancelToken::new()) {
        f.unwrap();
        count += 1;
    }
    assert_eq!(count, n);
    let g = open(&dir.join("grad-grey16-I.flif"));
    assert_eq!(g.colour_model(), Some("Greyscale"));
    assert_eq!(rgba(g.as_ref()).3, PixelLayout::Gray16);
    let t = open(&dir.join("grad-rgb10-N.flif"));
    assert!(
        t.info()
            .coding
            .starts_with("FLIF 10-bit RGB, non-interlaced"),
        "{}",
        t.info().coding
    );
    let m = open(&dir.join("meta.flif"));
    assert!(m.embedded_icc().is_some());
}

#[test]
fn corpus_flif_and_bpg_open() {
    let reg = Registry::new();
    for (sub, ext) in [("flif", "flif"), ("bpg", "bpg")] {
        let Some(dir) = corpus(sub) else {
            eprintln!("{sub} corpus absent: skipped");
            continue;
        };
        let (mut ok, mut refused) = (0, 0);
        for f in files(&dir, &[ext]) {
            match reg.open_path(&f) {
                Ok(s) => match s.decode(&DecodeRequest::full(1), &CancelToken::new()) {
                    Ok(_) => ok += 1,
                    Err(e) => {
                        refused += 1;
                        eprintln!("{}: {e}", f.display());
                    }
                },
                Err(e) => {
                    refused += 1;
                    eprintln!("{}: {e}", f.display());
                }
            }
        }
        eprintln!("{sub}: {ok} decoded, {refused} refused or failed");
    }
}
