//! The colour-grading maths against colour-science (BSD, a developer-run oracle): OkLab from linear Rec.2020 (and back), and the LUT spaces' primaries (Rec.2020 →
//! ACEScg with Bradford, → Rec.709) and curves (sRGB, ACEScct), ≤ 1e-6.
//!
//!     SPECULAR_COLOUR_PY=<venv>/bin/python \
//!       cargo test -p specular-develop --test oklab_oracle -- --ignored --nocapture
//!
//! The venv needs `colour-science` (`pip install colour-science`).

use std::process::Command;

use specular_develop::colour::oklab;
use specular_develop::ops::colour::lut::LutSpace;

const SCRIPT: &str = r#"
import json, sys, warnings
warnings.filterwarnings("ignore")
import numpy as np
import colour
d = json.loads(sys.stdin.read())
cs = colour.RGB_COLOURSPACES
bt = cs["ITU-R BT.2020"]
rgb = np.array(d["rgb"])
xyz = colour.RGB_to_XYZ(rgb, bt, illuminant=bt.whitepoint, chromatic_adaptation_transform=None)
lab = colour.XYZ_to_Oklab(xyz)
back = colour.XYZ_to_RGB(colour.Oklab_to_XYZ(lab), bt, illuminant=bt.whitepoint, chromatic_adaptation_transform=None)
ap1 = colour.matrix_RGB_to_RGB(bt, cs["ACEScg"], chromatic_adaptation_transform="Bradford")
r709 = colour.matrix_RGB_to_RGB(bt, cs["ITU-R BT.709"], chromatic_adaptation_transform=None)
x = np.array(d["curve"])
print(json.dumps({
    "lab": lab.tolist(), "back": back.tolist(), "ap1": ap1.tolist(), "r709": r709.tolist(),
    "srgb": colour.models.eotf_inverse_sRGB(x).tolist(),
    "acescct": colour.models.log_encoding_ACEScct(x).tolist(),
    "version": colour.__version__,
}))
"#;

fn rgb_samples() -> Vec<[f64; 3]> {
    let mut v = vec![
        [1.0, 1.0, 1.0],
        [0.18, 0.18, 0.18],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.05, 0.3, 0.9],
        [12.0, 6.0, 0.5],
        [0.001, 0.002, 0.0005],
    ];
    for i in 0..40 {
        let t = i as f64 / 39.0;
        v.push([t, (t * 7.0).sin().abs(), 1.0 - t * t]);
    }
    v
}

/// The ACEScct encoding the LUT op uses (the published S-2016-001 constants).
fn acescct(x: f64) -> f64 {
    if x <= 0.0078125 {
        10.540_237_741_654_5 * x + 0.072_905_534_195_835_5
    } else {
        (x.log2() + 9.72) / 17.52
    }
}

fn srgb(x: f64) -> f64 {
    if x <= 0.003_130_8 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

fn arrays(json: &str, key: &str) -> Vec<f64> {
    let at = json.find(&format!("\"{key}\":")).expect(key) + key.len() + 3;
    let mut depth = 0;
    let mut end = at;
    for (i, c) in json[at..].char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    end = at + i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    json[at..end]
        .split(|c: char| c == '[' || c == ']' || c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap())
        .collect()
}

#[test]
#[ignore = "needs colour-science: SPECULAR_COLOUR_PY=<venv>/bin/python"]
fn oklab_and_lut_spaces_match_colour_science() {
    let Some(py) = std::env::var_os("SPECULAR_COLOUR_PY") else {
        eprintln!("SPECULAR_COLOUR_PY not set: skipped");
        return;
    };
    let rgb = rgb_samples();
    let curve: Vec<f64> = (0..60).map(|i| (i as f64 * 0.25 - 12.0).exp2()).collect();
    let input = format!(
        "{{\"rgb\": {:?}, \"curve\": {:?}}}",
        rgb.iter().map(|c| c.to_vec()).collect::<Vec<_>>(),
        curve
    );
    let mut child = Command::new(py)
        .arg("-I")
        .arg("-c")
        .arg(SCRIPT)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("python runs");
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let json = String::from_utf8(out.stdout).unwrap();
    let version = json
        .split("\"version\": \"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .unwrap_or("?");
    let want_lab = arrays(&json, "lab");
    let want_back = arrays(&json, "back");
    let mut worst = [0f64; 5];
    for (i, c) in rgb.iter().enumerate() {
        let l = oklab::from_rec2020(*c);
        let b = oklab::to_rec2020(l);
        for k in 0..3 {
            worst[0] = worst[0].max((l[k] - want_lab[i * 3 + k]).abs());
            worst[1] = worst[1].max((b[k] - want_back[i * 3 + k]).abs() / c[k].abs().max(1.0));
        }
    }
    for (key, space) in [("ap1", LutSpace::Acescg), ("r709", LutSpace::Srgb)] {
        let want = arrays(&json, key);
        let m = space.to_primaries();
        for i in 0..9 {
            worst[2] = worst[2].max((m[i / 3][i % 3] - want[i]).abs());
        }
    }
    let ws = arrays(&json, "srgb");
    let wc = arrays(&json, "acescct");
    for (i, x) in curve.iter().enumerate() {
        worst[3] = worst[3].max((srgb(*x) - ws[i]).abs());
        worst[4] = worst[4].max((acescct(*x) - wc[i]).abs());
    }
    eprintln!(
        "vs colour-science {version}: OkLab {:.1e}, back to Rec.2020 {:.1e}, primaries {:.1e}, \
         sRGB curve {:.1e}, ACEScct {:.1e} ({} colours, {} curve values)",
        worst[0],
        worst[1],
        worst[2],
        worst[3],
        worst[4],
        rgb.len(),
        curve.len()
    );
    assert!(worst.iter().all(|w| *w <= 1e-6), "{worst:?}");
}
