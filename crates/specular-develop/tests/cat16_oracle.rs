//! CAT16 and Bradford against colour-science (BSD, a developer-run
//! oracle): the von Kries matrices between pairs of whites and adapted samples, ≤ 1e-6.
//!
//!     SPECULAR_COLOUR_PY=<venv>/bin/python \
//!       cargo test -p specular-develop --test cat16_oracle -- --ignored --nocapture
//!
//! The venv needs `colour-science` (`pip install colour-science`).

use std::process::Command;

use specular_develop::colour::{self, Cat};

/// Whites (xy): illuminant A, D50, D65, F2, a shade-like 7500 K, a magenta-ish off-locus one.
const WHITES: [[f64; 2]; 6] = [
    [0.44757, 0.40745],
    [0.3457, 0.3585],
    [0.3127, 0.3290],
    [0.37208, 0.37529],
    [0.2991, 0.3150],
    [0.3300, 0.3100],
];

const SAMPLES: [[f64; 3]; 4] = [
    [0.20654, 0.12197, 0.05136],
    [0.95047, 1.0, 1.08883],
    [0.01, 0.02, 0.5],
    [0.4, 0.35, 0.02],
];

const SCRIPT: &str = r#"
import json, sys, warnings
warnings.filterwarnings("ignore")
import numpy as np
import colour
d = json.loads(sys.stdin.read())
out = []
for t in ("CAT16", "Bradford"):
    for a in d["whites"]:
        for b in d["whites"]:
            wa = colour.xy_to_XYZ(np.array(a))
            wb = colour.xy_to_XYZ(np.array(b))
            m = colour.adaptation.matrix_chromatic_adaptation_VonKries(wa, wb, transform=t)
            s = [colour.adaptation.chromatic_adaptation_VonKries(np.array(x), wa, wb, transform=t).tolist() for x in d["samples"]]
            out.append({"a": a, "b": b, "m": m.tolist(), "s": s})
print(json.dumps(out))
"#;

#[test]
#[ignore = "needs colour-science: SPECULAR_COLOUR_PY=<venv>/bin/python"]
fn cat16_and_bradford_match_colour_science() {
    let Some(py) = std::env::var_os("SPECULAR_COLOUR_PY") else {
        eprintln!("SPECULAR_COLOUR_PY not set: skipped");
        return;
    };
    let input = format!(
        "{{\"whites\": {:?}, \"samples\": {:?}}}",
        WHITES.map(|w| w.to_vec()),
        SAMPLES.map(|s| s.to_vec())
    );
    let mut child = Command::new(py)
        .args(["-I", "-c", SCRIPT])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("python");
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let out = child.wait_with_output().expect("python output");
    assert!(out.status.success(), "colour-science failed");
    let text = String::from_utf8(out.stdout).unwrap();
    // A tiny reader for the flat JSON: every number in order.
    let nums: Vec<f64> = text
        .split(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | 'e' | 'E' | '+')))
        .filter(|t| !t.is_empty() && t.chars().any(|c| c.is_ascii_digit()))
        .map(|t| t.parse().expect("number"))
        .collect();
    // Per case: a (2), b (2), m (9), s (4×3).
    let per = 2 + 2 + 9 + SAMPLES.len() * 3;
    let cases = 2 * WHITES.len() * WHITES.len();
    assert_eq!(nums.len(), per * cases, "{}", &text[..text.len().min(400)]);
    let mut worst_m = 0f64;
    let mut worst_s = 0f64;
    let mut over = 0;
    for (i, c) in nums.chunks_exact(per).enumerate() {
        let cat = if i < cases / 2 {
            Cat::Cat16
        } else {
            Cat::Bradford
        };
        let ours = colour::adaptation_xyz(
            cat,
            colour::xy_to_xyz([c[0], c[1]]),
            colour::xy_to_xyz([c[2], c[3]]),
        );
        for k in 0..9 {
            let e = (ours[k / 3][k % 3] - c[4 + k]).abs();
            worst_m = worst_m.max(e);
            over += usize::from(e > 1e-6);
        }
        for (j, x) in SAMPLES.iter().enumerate() {
            let y = colour::mulv(&ours, *x);
            for k in 0..3 {
                let e = (y[k] - c[13 + j * 3 + k]).abs();
                worst_s = worst_s.max(e);
                over += usize::from(e > 1e-6);
            }
        }
    }
    eprintln!(
        "CAT16 + Bradford vs colour-science: {cases} white pairs, worst matrix |Δ| {worst_m:e}, worst sample |Δ| {worst_s:e}, {over} over 1e-6"
    );
    assert_eq!(over, 0);
}
