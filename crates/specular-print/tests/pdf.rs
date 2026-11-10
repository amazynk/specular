//! The PDF writer read back by a strict test-only parser (and `qpdf --check` when
//! installed): structure, colour space and output intent, and every pixel of every page.

mod support;

use specular_print::pdf::{Meta, PdfWriter};

fn pattern(w: u32, h: u32, seed: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            // Mixed content: smooth areas and noise, so compression has work to do.
            let n = (x.wrapping_mul(2_654_435_761) ^ y.wrapping_mul(40_503) ^ seed) >> 13;
            let noisy = (x / 32 + y / 32) % 3 == 0;
            v.extend_from_slice(&[
                if noisy { n as u8 } else { (x * 255 / w) as u8 },
                (y * 255 / h) as u8,
                (seed * 70) as u8,
            ]);
        }
    }
    v
}

fn out_dir() -> std::path::PathBuf {
    let d = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("specular-print");
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn streamed_pages_read_back_exactly() {
    let meta = Meta {
        title: "Contact sheet — Ferien (2026)".into(),
        date: specular_print::pdf::date(1_791_590_400),
        ..Default::default()
    };
    let mut w = PdfWriter::new(Vec::new(), &meta).unwrap();
    let pages = [
        ((595.276, 841.89), 210, 297),
        ((841.89, 595.276), 297, 210),
        ((612.0, 792.0), 1, 1),
    ];
    let mut expect = Vec::new();
    for (i, &(pt, pw, ph)) in pages.iter().enumerate() {
        let rgb = pattern(pw, ph, i as u32);
        w.begin_page(pt, pw, ph).unwrap();
        // Uneven bands: 1 row, 7 rows, then the rest in 100-row bands.
        let row = pw as usize * 3;
        let mut at = 0;
        for band in [1usize, 7].into_iter().chain(std::iter::repeat(100)) {
            if at >= ph as usize {
                break;
            }
            let n = band.min(ph as usize - at);
            w.write_rows(&rgb[at * row..(at + n) * row]).unwrap();
            at += n;
        }
        w.end_page().unwrap();
        expect.push(rgb);
    }
    let (bytes, stats) = w.finish().unwrap();
    assert_eq!(stats.pages, 3);
    assert_eq!(stats.bytes as usize, bytes.len());

    let pdf = support::parse(&bytes);
    assert_eq!(pdf.version, "1.4");
    assert_eq!(pdf.icc, specular_print::icc::srgb_v2());
    assert!(pdf.intent.contains("(sRGB IEC61966-2.1)"));
    assert!(pdf.info.contains("/Title <FEFF"), "{}", pdf.info);
    assert!(pdf.info.contains("(D:20261010000000Z)"));
    assert_eq!(pdf.pages.len(), 3);
    for (p, (&(pt, pw, ph), rgb)) in pdf.pages.iter().zip(pages.iter().zip(&expect)) {
        assert_eq!((p.width, p.height), (pw, ph));
        assert!((p.media_box[2] - pt.0).abs() < 0.001 && (p.media_box[3] - pt.1).abs() < 0.001);
        assert_eq!(p.media_box[0], 0.0);
        assert!(p.content.contains("cm /Im0 Do"), "{}", p.content);
        assert!(&p.rgb == rgb, "page pixels survive the round trip");
    }

    let path = out_dir().join("streamed.pdf");
    std::fs::write(&path, &bytes).unwrap();
    assert!(support::qpdf_ok(&path));
}

#[test]
fn a_document_without_pages_is_still_valid() {
    let w = PdfWriter::new(Vec::new(), &Meta::default()).unwrap();
    let (bytes, stats) = w.finish().unwrap();
    assert_eq!(stats.pages, 0);
    let pdf = support::parse(&bytes);
    assert!(pdf.pages.is_empty());
    assert!(!pdf.info.contains("/Title"));
}

#[test]
fn large_page_compresses_in_bounded_memory() {
    // An A4 page at 300 dpi of flat white streams through without holding the page: the
    // writer only ever sees one band.
    let (pw, ph) = (2480u32, 3508u32);
    let mut w = PdfWriter::new(Vec::new(), &Meta::default()).unwrap();
    w.begin_page((595.276, 841.89), pw, ph).unwrap();
    let band = vec![255u8; pw as usize * 3 * 512];
    let mut left = ph as usize;
    while left > 0 {
        let n = left.min(512);
        w.write_rows(&band[..n * pw as usize * 3]).unwrap();
        left -= n;
    }
    w.end_page().unwrap();
    let (bytes, _) = w.finish().unwrap();
    // 26 MB of white compresses to well under 1 %.
    assert!(bytes.len() < 200_000, "{} bytes", bytes.len());
    let pdf = support::parse(&bytes);
    assert!(pdf.pages[0].rgb.iter().all(|&v| v == 255));
}

/// Printing through a printer profile (Develop's soft-proof profile): the profile is
/// the colour space and the output intent, and the pixels are its device values.
#[test]
fn pages_print_through_a_printer_profile() {
    use specular_color::transform::Intent;
    use specular_print::profile::PrintProfile;
    use std::sync::Arc;
    let cmyk = include_bytes!("../../specular-color/fuzz/seeds/profile/cmyk_v2_lut16.icc");
    let rgb_icc = include_bytes!(
        "../../formats/specular-jxl/src/color/icc/test-profiles/prophoto-gamma18-rel.icc"
    );
    for (icc, n, intent) in [
        (&cmyk[..], 4usize, Intent::Perceptual),
        (&rgb_icc[..], 3, Intent::Relative),
    ] {
        let profile = Arc::new(PrintProfile::new(icc.to_vec(), intent, true).unwrap());
        let meta = Meta {
            title: "proof".into(),
            output: Some(profile.clone()),
            ..Default::default()
        };
        let (pw, ph) = (64u32, 40u32);
        let rgb = pattern(pw, ph, 3);
        let mut w = PdfWriter::new(Vec::new(), &meta).unwrap();
        w.begin_page((200.0, 125.0), pw, ph).unwrap();
        w.write_rows(&rgb[..pw as usize * 3 * 10]).unwrap();
        w.write_rows(&rgb[pw as usize * 3 * 10..]).unwrap();
        w.end_page().unwrap();
        let (bytes, _) = w.finish().unwrap();
        let pdf = support::parse(&bytes);
        assert_eq!(pdf.icc, icc, "the printer profile is embedded");
        assert_eq!(pdf.channels, n);
        assert!(
            pdf.intent.contains(&format!("({})", profile.name)),
            "{}",
            pdf.intent
        );
        let mut want = Vec::new();
        profile.convert_row(&rgb, &mut want);
        assert_eq!(
            pdf.pages[0].rgb, want,
            "device values of the converted rows"
        );
        // Close to the exact transform.
        let worst = rgb
            .chunks_exact(3)
            .zip(want.chunks_exact(n))
            .map(|(c, d)| {
                let e = profile.exact([c[0], c[1], c[2]]);
                e.iter().zip(d).map(|(a, b)| a.abs_diff(*b)).max().unwrap()
            })
            .max()
            .unwrap();
        assert!(worst <= 3, "worst {worst} codes");
        let path = out_dir().join(format!("proof-{n}.pdf"));
        std::fs::write(&path, &bytes).unwrap();
        assert!(support::qpdf_ok(&path));
    }
}
