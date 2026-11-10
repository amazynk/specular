//! Damaged input: every truncation and about 2000 byte flips of the fixtures parse or
//! fail cleanly, quickly.

use std::time::{Duration, Instant};

use specular_cad::{Limits, parse, parse_header_only};

const FIXTURES: &[&[u8]] = &[
    include_bytes!("data/entities.dxf"),
    include_bytes!("data/r12.dxf"),
    include_bytes!("data/layout.dxf"),
];

/// Generous for a debug build of a few-KB file; a hang or blow-up shows far above it.
const BUDGET: Duration = Duration::from_secs(2);

fn check(bytes: &[u8], what: &str) {
    let t = Instant::now();
    let _ = parse(bytes, &Limits::default());
    let _ = parse_header_only(bytes);
    assert!(t.elapsed() < BUDGET, "{what}: {:?}", t.elapsed());
}

#[test]
fn truncations() {
    for (i, f) in FIXTURES.iter().enumerate() {
        for n in 0..f.len() {
            check(&f[..n], &format!("fixture {i} cut at {n}"));
        }
    }
}

#[test]
fn byte_flips() {
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    // Values that steer parsing: digits, signs, newlines, group-code-looking text.
    let interesting = b"0123456789-+.eE\r\n \x00\xff";
    for round in 0..2000 {
        let f = FIXTURES[round % FIXTURES.len()];
        let mut b = f.to_vec();
        for _ in 0..1 + next() % 4 {
            let at = (next() % b.len() as u64) as usize;
            b[at] = if next() % 2 == 0 {
                interesting[(next() % interesting.len() as u64) as usize]
            } else {
                next() as u8
            };
        }
        check(&b, &format!("round {round}"));
    }
}

#[test]
fn hostile_numbers() {
    // Huge counts, sizes and repeats must not allocate or loop on their say-so.
    let cases: &[&[u8]] = &[
        b"0\nSECTION\n2\nENTITIES\n0\nINSERT\n2\nX\n70\n2000000000\n71\n2000000000\n0\nENDSEC\n0\nEOF\n",
        b"0\nSECTION\n2\nENTITIES\n0\nLWPOLYLINE\n90\n2000000000\n0\nENDSEC\n0\nEOF\n",
        b"0\nSECTION\n2\nENTITIES\n0\nCIRCLE\n10\n1e308\n20\n-1e308\n40\n1e308\n0\nENDSEC\n0\nEOF\n",
        b"0\nSECTION\n2\nENTITIES\n0\nARC\n10\nnan\n20\ninf\n40\n1\n50\n0\n51\n1e300\n0\nENDSEC\n0\nEOF\n",
        b"0\nSECTION\n2\nENTITIES\n0\nMESH\n92\n1\n10\n0\n20\n0\n30\n0\n93\n3\n90\n2000000000\n90\n0\n90\n0\n0\nENDSEC\n0\nEOF\n",
        b"0\nSECTION\n2\nENTITIES\n0\nHATCH\n2\nANSI31\n70\n0\n91\n1\n92\n1\n93\n2000000000\n72\n1\n10\n0\n20\n0\n11\n1\n21\n1\n78\n1\n53\n45\n43\n0\n44\n0\n45\n0\n46\n1e-300\n0\nENDSEC\n0\nEOF\n",
        b"0\nSECTION\n2\nENTITIES\n0\nSPLINE\n71\n2000000000\n72\n3\n40\n0\n40\n1\n40\n2\n10\n0\n20\n0\n0\nENDSEC\n0\nEOF\n",
        b"0\nSECTION\n2\nENTITIES\n0\nIMAGE\n10\n0\n20\n0\n11\n1e-300\n21\n0\n12\n0\n22\n1e300\n13\n1e300\n23\n1\n0\nENDSEC\n0\nEOF\n",
    ];
    for (i, c) in cases.iter().enumerate() {
        check(c, &format!("case {i}"));
    }
}

// ------------------------------------------------------------------- per-file budgets

fn dxf(blocks: &str, entities: &str) -> Vec<u8> {
    format!(
        "0\nSECTION\n2\nBLOCKS\n{blocks}0\nENDSEC\n0\nSECTION\n2\nENTITIES\n{entities}0\nENDSEC\n0\nEOF\n"
    )
    .into_bytes()
}

/// Path elements of every item of every sheet.
fn elements(s: &specular_vector::VectorScene) -> usize {
    use specular_vector::Shape;
    s.sheets
        .iter()
        .flat_map(|sh| &sh.items)
        .map(|it| match &it.shape {
            Shape::Stroke { path, .. } | Shape::Fill { path, .. } => path.elements().len(),
            _ => 1,
        })
        .sum()
}

/// A 1000 × 1000 square hatched with horizontal lines `d` apart.
fn hatch(d: f64) -> String {
    let mut s = String::from("0\nHATCH\n8\n0\n2\nUSER\n70\n0\n71\n0\n91\n1\n92\n1\n93\n4\n");
    for (a, b) in [
        ((0, 0), (1000, 0)),
        ((1000, 0), (1000, 1000)),
        ((1000, 1000), (0, 1000)),
        ((0, 1000), (0, 0)),
    ] {
        s += &format!(
            "72\n1\n10\n{}\n20\n{}\n11\n{}\n21\n{}\n",
            a.0, a.1, b.0, b.1
        );
    }
    s += &format!(
        "97\n0\n75\n0\n76\n0\n52\n0\n41\n1\n77\n0\n78\n1\n53\n0\n43\n0\n44\n0\n45\n0\n46\n{d}\n79\n0\n"
    );
    s
}

#[test]
fn hatches_share_one_budget() {
    // One dense hatch is drawn (about 150 000 lines)...
    let one = parse(&dxf("", &hatch(1000.0 / 150_000.0)), &Limits::default()).unwrap();
    assert!(elements(&one) > 250_000, "{}", elements(&one));
    // ...but 60 of them (about 18 million path elements before) stop at the file's budget.
    let t = Instant::now();
    let many = parse(
        &dxf("", &hatch(1000.0 / 150_000.0).repeat(60)),
        &Limits::default(),
    )
    .unwrap();
    assert!(elements(&many) <= 8_400_000, "{}", elements(&many));
    assert!(t.elapsed() < Duration::from_secs(60));
}

#[test]
fn insert_arrays_share_one_budget() {
    // Sixty 1000 × 100 arrays of an empty block: six million transforms (and copies past
    // the item limit) before; past a million cells, arrays are placed once.
    let block = "0\nBLOCK\n8\n0\n2\nX\n70\n0\n10\n0\n20\n0\n0\nENDBLK\n";
    let insert = "0\nINSERT\n8\n0\n2\nX\n70\n1000\n71\n100\n44\n1\n45\n1\n";
    let s = parse(&dxf(block, &insert.repeat(60)), &Limits::default()).unwrap();
    assert!(
        s.warnings
            .iter()
            .any(|w| w.contains("INSERT arrays past the limit")),
        "{:?}",
        s.warnings
    );
}

#[test]
fn copies_of_a_large_block_are_bounded() {
    // A block of one 200 000-vertex polyline placed 200 times: 40 million path elements
    // before; the drawing's element budget refuses it.
    let mut pl = String::from("0\nLWPOLYLINE\n8\n0\n90\n200000\n70\n0\n");
    for k in 0..200_000 {
        pl += &format!("10\n{}\n20\n0\n", k % 1000);
    }
    let block = format!("0\nBLOCK\n8\n0\n2\nX\n70\n0\n10\n0\n20\n0\n{pl}0\nENDBLK\n");
    let insert = "0\nINSERT\n8\n0\n2\nX\n10\n0\n20\n0\n";
    let e = parse(&dxf(&block, &insert.repeat(200)), &Limits::default()).unwrap_err();
    assert_eq!(e, specular_cad::CadError::TooLarge);
}
