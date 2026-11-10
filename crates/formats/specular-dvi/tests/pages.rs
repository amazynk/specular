//! Hand-built DVI files: the preamble, pages, fonts, rules and character positions; and the
//! dvitype oracle (tools/corpus/dvi-oracle.py) when `SPECULAR_DVI_CORPUS` is set.

use specular_dvi::fixtures::sample;
use specular_dvi::{Error, Item, page, parse, tfm};

fn u32b(v: u32) -> [u8; 4] {
    v.to_be_bytes()
}

#[test]
fn structure_and_items() {
    let d = sample();
    let dvi = parse(&d).unwrap();
    assert_eq!(dvi.comment, "TeX output 2026.11.07");
    assert_eq!(dvi.pages.len(), 2);
    assert_eq!(dvi.page_numbers, vec![1, 2]);
    assert_eq!(dvi.fonts[&0].name, "cmr10");
    assert_eq!((dvi.max_v, dvi.max_h), (1_000_000, 2_000_000));
    assert!(dvi.complete);
    // Without metrics a character advances half the font size.
    let items = page(&dvi, &d, 0, &|_, _| None).unwrap();
    assert_eq!(
        items,
        vec![
            Item::Char {
                font: 0,
                code: 65,
                x: 0,
                y: 100
            },
            Item::Char {
                font: 0,
                code: 66,
                x: 327_680,
                y: 100
            },
            Item::Rule {
                x: 0,
                y: 0,
                w: 50_000,
                h: 1000
            },
        ]
    );
    // With widths from metrics.
    let items = page(&dvi, &d, 0, &|_, c| Some(c as i64 * 10)).unwrap();
    assert!(matches!(items[1], Item::Char { x: 650, .. }));
    let items = page(&dvi, &d, 1, &|_, _| None).unwrap();
    assert_eq!(items.len(), 1);
    assert!(page(&dvi, &d, 2, &|_, _| None).is_err());
}

#[test]
fn damaged_files() {
    assert_eq!(parse(b"hello world, not dvi"), Err(Error::NotDvi));
    let d = sample();
    // Cut before the postamble: the pages still read.
    // post_post's pointer gives the postamble.
    let n = d.len();
    let post = u32::from_be_bytes(d[n - 9..n - 5].try_into().unwrap()) as usize;
    let dvi = parse(&d[..post]).unwrap();
    assert!(!dvi.complete);
    assert_eq!(dvi.pages.len(), 2);
    for n in 0..d.len() {
        let cut = &d[..n];
        if let Ok(dvi) = parse(cut) {
            for p in 0..dvi.pages.len() {
                let _ = page(&dvi, cut, p, &|_, _| None);
            }
        }
    }
    // A pop without a push.
    let mut bad = d.clone();
    let at = bad.iter().position(|&b| b == 141).unwrap();
    bad[at] = 138;
    let dvi = parse(&bad).unwrap();
    assert!(page(&dvi, &bad, 0, &|_, _| None).is_err());
}

#[test]
fn tfm_metrics() {
    // lf lh bc ec nw nh nd ni nl nk ne np; header 2 words; 1 char; 2 widths, 2 heights,
    // 1 depth.
    let halves = [14u16, 2, 65, 65, 2, 2, 1, 0, 0, 0, 0, 0];
    let mut d: Vec<u8> = halves.iter().flat_map(|h| h.to_be_bytes()).collect();
    d.extend_from_slice(&u32b(0xABCD));
    d.extend_from_slice(&u32b(10 << 20));
    d.extend_from_slice(&[1, 0x10, 0, 0]);
    d.extend_from_slice(&u32b(0));
    d.extend_from_slice(&u32b(1 << 19)); // width 0.5
    d.extend_from_slice(&u32b(0));
    d.extend_from_slice(&u32b(1 << 18)); // height 0.25
    d.extend_from_slice(&u32b(0));
    let t = tfm::parse(&d).unwrap();
    assert_eq!(t.checksum, 0xABCD);
    assert_eq!(t.dims(65, 655_360), Some((327_680, 163_840, 0)));
    assert_eq!(t.dims(66, 655_360), None);
    assert!(tfm::parse(&d[..20]).is_none());
}

/// dvitype's positions for every `setchar`/`setrule` against ours.
#[test]
#[ignore]
fn dvitype_oracle() {
    let Some(root) = std::env::var_os("SPECULAR_DVI_CORPUS") else {
        eprintln!("SPECULAR_DVI_CORPUS not set");
        return;
    };
    let root = std::path::Path::new(&root);
    let mut total = 0;
    let mut mismatches = 0;
    for name in ["plain", "rules"] {
        let d = std::fs::read(root.join(format!("{name}.dvi"))).unwrap();
        let expected = std::fs::read_to_string(root.join(format!("{name}.positions"))).unwrap();
        let dvi = parse(&d).unwrap();
        let mut fonts = std::collections::HashMap::new();
        for (k, f) in &dvi.fonts {
            let t = std::fs::read(root.join(format!("{}.tfm", f.name)))
                .ok()
                .and_then(|b| tfm::parse(&b));
            fonts.insert(*k, (t, f.scale));
        }
        let width = |f: u32, c: u32| {
            let (t, s) = fonts.get(&f)?;
            t.as_ref()?.dims(c, *s).map(|d| d.0)
        };
        let mut ours = String::new();
        for p in 0..dvi.pages.len() {
            for it in page(&dvi, &d, p, &width).unwrap() {
                match it {
                    Item::Char { code, x, y, .. } => {
                        ours.push_str(&format!("{p} c {code} {x} {y}\n"))
                    }
                    Item::Rule { x, y, w, h } => ours.push_str(&format!("{p} r {x} {y} {w} {h}\n")),
                }
            }
        }
        let a: Vec<&str> = ours.lines().collect();
        let b: Vec<&str> = expected.lines().collect();
        total += b.len();
        mismatches += a.iter().zip(&b).filter(|(x, y)| x != y).count() + a.len().abs_diff(b.len());
        eprintln!("{name}: {} items, ours {}", b.len(), a.len());
    }
    eprintln!("dvitype: {mismatches} mismatches in {total} items");
    assert_eq!(mismatches, 0);
}
