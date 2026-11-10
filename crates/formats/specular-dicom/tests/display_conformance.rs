//! The grey display pipeline (PS3.3 C.11: modality rescale or LUT, window with the LINEAR,
//! LINEAR_EXACT or SIGMOID function or a VOI LUT, Presentation LUT, Presentation LUT Shape
//! and MONOCHROME1) against pydicom, and the GSDF (PS3.14) against colour-science. The
//! oracles are tests/data/display_oracle.tsv and gsdf_oracle.tsv from
//! tools/quality/dicom_reference.py; each display case is a one-row image built here with the
//! crate's writer, decoded and rendered with its default VOI.

use specular_dicom::dict::Vr;
use specular_dicom::write::{self, FileMeta, Object, Value};
use specular_dicom::{Dicom, Rendered, gsdf};

type Elem = (u32, Vr, Vec<u8>);

struct Case<'a> {
    name: &'a str,
    photometric: &'a str,
    elems: Vec<Elem>,
    seqs: Vec<(u32, Vec<Elem>)>,
    stored: Vec<i64>,
    expect: Vec<f64>,
}

fn vr(s: &str) -> Vr {
    match s {
        "US" => Vr::US,
        "SS" => Vr::SS,
        "DS" => Vr::DS,
        "CS" => Vr::CS,
        _ => panic!("VR {s}"),
    }
}

fn elem(f: &[&str]) -> Elem {
    let tag = u32::from_str_radix(f[0], 16).unwrap();
    let v = vr(f[1]);
    let bytes = match v {
        Vr::US | Vr::SS => f[2]
            .split(' ')
            .flat_map(|x| (x.parse::<i64>().unwrap() as u16).to_le_bytes())
            .collect(),
        _ => {
            let mut b = f[2].as_bytes().to_vec();
            if b.len() % 2 == 1 {
                b.push(b' ');
            }
            b
        }
    };
    (tag, v, bytes)
}

fn cases(text: &str) -> Vec<Case<'_>> {
    let mut out: Vec<Case> = Vec::new();
    let mut seq: Option<(u32, Vec<Elem>)> = None;
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let f: Vec<&str> = line.split('\t').collect();
        let c = out.last_mut();
        match f[0] {
            "case" => out.push(Case {
                name: f[1],
                photometric: f[2],
                elems: Vec::new(),
                seqs: Vec::new(),
                stored: Vec::new(),
                expect: Vec::new(),
            }),
            "elem" => c.unwrap().elems.push(elem(&f[1..])),
            "" => seq.as_mut().unwrap().1.push(elem(&f[2..])),
            "seq" => seq = Some((u32::from_str_radix(f[1], 16).unwrap(), Vec::new())),
            "end" => c.unwrap().seqs.push(seq.take().unwrap()),
            "stored" => c.unwrap().stored = f[1].split(' ').map(|x| x.parse().unwrap()).collect(),
            "expect" => c.unwrap().expect = f[1].split(' ').map(|x| x.parse().unwrap()).collect(),
            k => panic!("line kind {k}"),
        }
    }
    out
}

fn file(c: &Case) -> Vec<u8> {
    let mut o = Object::default();
    o.put_text(0x0008_0016, Vr::UI, "1.2.840.10008.5.1.4.1.1.7");
    o.put_text(0x0008_0018, Vr::UI, "1.2.826.0.1.3680043.10.1.77");
    o.put_bytes(0x0028_0002, Vr::US, 1u16.to_le_bytes());
    o.put_text(0x0028_0004, Vr::CS, c.photometric);
    o.put_bytes(0x0028_0010, Vr::US, 1u16.to_le_bytes());
    o.put_bytes(0x0028_0011, Vr::US, (c.stored.len() as u16).to_le_bytes());
    for (tag, vr, b) in &c.elems {
        o.put_bytes(*tag, *vr, b.clone());
    }
    for (tag, items) in &c.seqs {
        let mut item = Object::default();
        for (t, vr, b) in items {
            item.put_bytes(*t, *vr, b.clone());
        }
        o.put(*tag, Vr::SQ, Value::Items(vec![item]));
    }
    let px: Vec<u8> = c
        .stored
        .iter()
        .flat_map(|&s| (s as u16).to_le_bytes())
        .collect();
    o.put_bytes(0x7FE0_0010, Vr::OW, px);
    write::part10(&o, &FileMeta::of(&o, write::EXPLICIT_LE)).unwrap()
}

#[test]
fn display_pipeline_matches_pydicom() {
    let text = include_str!("data/display_oracle.tsv");
    let mut report = Vec::new();
    let mut total = 0;
    for c in cases(text) {
        assert_eq!(c.stored.len(), c.expect.len(), "{}", c.name);
        let f = file(&c);
        let d = Dicom::parse(&f).unwrap();
        let m = d.modality_frame(&f, 0, None).unwrap();
        let img = m.render_default().unwrap();
        let Rendered::Gray16(px) = img.pixels else {
            panic!("{}: not grey", c.name)
        };
        let mut worst = 0f64;
        for (i, (&s, &want)) in c.stored.iter().zip(&c.expect).enumerate() {
            // Rendering rounds the display fraction to 16 bits.
            let err = (px[i] as f64 - want * 65535.0).abs();
            worst = worst.max(err);
            if err > 0.5 + 1e-4 {
                report.push(format!(
                    "{}: stored {s}: got {} want {:.3}",
                    c.name,
                    px[i],
                    want * 65535.0
                ));
            }
        }
        total += c.stored.len();
        eprintln!(
            "{:<34} {:>4} values, worst {worst:.3} of 65535",
            c.name,
            c.stored.len()
        );
    }
    eprintln!("{total} values, {} mismatches", report.len());
    assert!(report.is_empty(), "{report:#?}");
}

#[test]
fn gsdf_matches_colour_science() {
    let mut worst_l = 0f64;
    let mut worst_j = 0f64;
    for line in include_str!("data/gsdf_oracle.tsv").lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let (a, b): (f64, f64) = match f[..] {
            [_, a, b] => (a.parse().unwrap(), b.parse().unwrap()),
            _ => continue,
        };
        match f[0] {
            "L" => worst_l = worst_l.max((gsdf::luminance(a) / b - 1.0).abs()),
            "J" => worst_j = worst_j.max((gsdf::jnd_index(a) - b).abs()),
            _ => {}
        }
    }
    eprintln!("GSDF: luminance rel. error {worst_l:.2e}, JND index error {worst_j:.2e}");
    assert!(worst_l < 1e-9 && worst_j < 1e-5);
}

/// P-values on a display calibrated between its black and white (the medical display
/// setting): equal JND steps, against colour-science.
#[test]
fn gsdf_p_values_match_colour_science() {
    let (mut worst, mut n) = (0f64, 0);
    for line in include_str!("data/gsdf_oracle.tsv").lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let ["P", lmin, lmax, p, l] = f[..] else {
            continue;
        };
        let v: Vec<f64> = [lmin, lmax, p, l]
            .iter()
            .map(|s| s.parse().unwrap())
            .collect();
        worst = worst.max((gsdf::p_luminance(v[2], v[0], v[1]) / v[3] - 1.0).abs());
        n += 1;
    }
    eprintln!("GSDF P-values: {n}, luminance rel. error {worst:.2e}");
    assert!(n == 260 && worst < 1e-8, "{n} values, worst {worst}");
}
