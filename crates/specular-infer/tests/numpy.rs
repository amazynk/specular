//! Independent references from numpy (float64) for the CPU oracle itself: matmul + bias +
//! exact GELU, grouped strided dilated padded conv2d, softmax, layer norm, causal
//! multi-head attention and half-pixel bilinear resize. The GPU is checked against the CPU
//! in the same run. Skipped when `python3` with numpy is not available (nothing is
//! installed). Tolerance against numpy: 1e-5 + 1e-5·|ref|.

mod common;

use std::path::Path;
use std::process::Command;

use common::check;
use specular_infer::graph::{Conv2d, MatMul, ResizeMode};
use specular_infer::tensor::{Rng, compare};
use specular_infer::{Act, DType, Feed, Graph, Tensor};

const SCRIPT: &str = r#"
import math, sys
import numpy as np

d = sys.argv[1]
def ld(name, shape):
    return np.fromfile(f"{d}/{name}.f32", dtype="<f4").astype(np.float64).reshape(shape)
def sv(name, a):
    np.asarray(a, dtype="<f8").tofile(f"{d}/{name}.ref")

# matmul [2,7,13] x W[9,13]^T + b, exact GELU
a, w, b = ld("a", (2, 7, 13)), ld("w", (9, 13)), ld("b", (9,))
y = a @ w.T + b
erf = np.vectorize(math.erf)
sv("mm", 0.5 * y * (1.0 + erf(y / math.sqrt(2.0))))

# conv2d x[2,4,11,9], k[6,2,3,3], groups 2, stride (2,1), pad t1 l2 b0 r1, dilation (1,2)
x, k, kb = ld("x", (2, 4, 11, 9)), ld("k", (6, 2, 3, 3)), ld("kb", (6,))
xp = np.pad(x, ((0, 0), (0, 0), (1, 0), (2, 1)))
G, sy, sx, dy, dx = 2, 2, 1, 1, 2
H, W = xp.shape[2], xp.shape[3]
OH = (H - (dy * 2 + 1)) // sy + 1
OW = (W - (dx * 2 + 1)) // sx + 1
out = np.zeros((2, 6, OH, OW))
og, cg = 3, 2
for g in range(G):
    for o in range(og):
        oc = g * og + o
        for i in range(3):
            for j in range(3):
                patch = xp[:, g * cg:(g + 1) * cg, i * dy:i * dy + sy * (OH - 1) + 1:sy,
                           j * dx:j * dx + sx * (OW - 1) + 1:sx]
                out[:, oc] += np.einsum("nchw,c->nhw", patch, k[oc, :, i, j])
        out[:, oc] += kb[oc]
sv("conv", out)

# softmax and layer norm of s[3,17]
s, gm, bt = ld("s", (3, 17)), ld("gm", (17,)), ld("bt", (17,))
e = np.exp(s - s.max(axis=-1, keepdims=True))
sv("softmax", e / e.sum(axis=-1, keepdims=True))
mu = s.mean(axis=-1, keepdims=True)
var = ((s - mu) ** 2).mean(axis=-1, keepdims=True)
sv("ln", (s - mu) / np.sqrt(var + 1e-5) * gm + bt)

# causal attention, q[2,5,8], k/v[2,7,8], 2 heads, query t sees keys 0..=t
q, kk, v = ld("q", (2, 5, 8)), ld("kk", (2, 7, 8)), ld("v", (2, 7, 8))
att = np.zeros((2, 5, 8))
for n in range(2):
    for h in range(2):
        qs, ks, vs = q[n, :, h*4:(h+1)*4], kk[n, :, h*4:(h+1)*4], v[n, :, h*4:(h+1)*4]
        sc = qs @ ks.T / math.sqrt(4)
        for t in range(5):
            row = sc[t, :min(t + 1, 7)]
            p = np.exp(row - row.max()); p /= p.sum()
            att[n, t, h*4:(h+1)*4] = p @ vs[:len(p)]
sv("att", att)

# bilinear resize of x to 5x14, half-pixel centres, edges clamped
def axis(n_in, n_out):
    c = np.clip((np.arange(n_out) + 0.5) * n_in / n_out - 0.5, 0, n_in - 1)
    i0 = np.floor(c).astype(int)
    return i0, np.minimum(i0 + 1, n_in - 1), c - i0
y0, y1, ty = axis(11, 5)
x0, x1, tx = axis(9, 14)
top = x[:, :, y0][:, :, :, x0] * (1 - tx) + x[:, :, y0][:, :, :, x1] * tx
bot = x[:, :, y1][:, :, :, x0] * (1 - tx) + x[:, :, y1][:, :, :, x1] * tx
sv("resize", top * (1 - ty[:, None]) + bot * ty[:, None])
"#;

fn save(dir: &Path, name: &str, t: &Tensor) {
    let bytes: Vec<u8> = t.to_f32().iter().flat_map(|v| v.to_le_bytes()).collect();
    std::fs::write(dir.join(format!("{name}.f32")), bytes).unwrap();
}

fn load(dir: &Path, name: &str) -> Vec<f32> {
    let b = std::fs::read(dir.join(format!("{name}.ref"))).unwrap();
    b.chunks_exact(8)
        .map(|c| f64::from_le_bytes(c.try_into().unwrap()) as f32)
        .collect()
}

#[test]
fn against_numpy() {
    let ok = Command::new("python3")
        .args(["-I", "-c", "import numpy"])
        .output()
        .is_ok_and(|o| o.status.success());
    if !ok {
        eprintln!("python3 with numpy not available: numpy references skipped");
        return;
    }
    let dir = std::env::temp_dir().join(format!("specular-infer-numpy-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut rng = Rng::new(42);
    let mut t = |name: &str, shape: &[usize], a: f32| {
        let v = rng.tensor(shape, a);
        save(&dir, name, &v);
        v
    };
    let (a, w, b) = (
        t("a", &[2, 7, 13], 1.0),
        t("w", &[9, 13], 0.3),
        t("b", &[9], 0.2),
    );
    let (x, k, kb) = (
        t("x", &[2, 4, 11, 9], 1.0),
        t("k", &[6, 2, 3, 3], 0.3),
        t("kb", &[6], 0.2),
    );
    let (s, gm, bt) = (
        t("s", &[3, 17], 4.0),
        t("gm", &[17], 1.0),
        t("bt", &[17], 0.5),
    );
    let (q, kk, v) = (
        t("q", &[2, 5, 8], 1.0),
        t("kk", &[2, 7, 8], 1.0),
        t("v", &[2, 7, 8], 1.0),
    );
    let script = dir.join("refs.py");
    std::fs::write(&script, SCRIPT).unwrap();
    let out = Command::new("python3")
        .arg("-I")
        .arg(&script)
        .arg(&dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "numpy script: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let mut g = Graph::new();
    let ai = g.input(&[2, 7, 13], DType::F32).unwrap();
    let xi = g.input(&[2, 4, 11, 9], DType::F32).unwrap();
    let si = g.input(&[3, 17], DType::F32).unwrap();
    let qi = g.input(&[2, 5, 8], DType::F32).unwrap();
    let ki = g.input(&[2, 7, 8], DType::F32).unwrap();
    let vi = g.input(&[2, 7, 8], DType::F32).unwrap();
    let (wc, bc) = (g.constant(w).unwrap(), g.constant(b).unwrap());
    let mm = g
        .matmul(
            ai,
            wc,
            Some(bc),
            MatMul {
                trans_b: true,
                act: Act::Gelu,
            },
        )
        .unwrap();
    let (kc, kbc) = (g.constant(k).unwrap(), g.constant(kb).unwrap());
    let p = Conv2d {
        stride: [2, 1],
        pad: [1, 2, 0, 1],
        dilation: [1, 2],
        groups: 2,
        act: Act::Identity,
    };
    let conv = g.conv2d(xi, kc, Some(kbc), p).unwrap();
    let sm = g.softmax(si).unwrap();
    let (gc, bc) = (g.constant(gm).unwrap(), g.constant(bt).unwrap());
    let ln = g.layer_norm(si, gc, bc, 1e-5).unwrap();
    let att = g.attention(qi, ki, vi, 2, None, true).unwrap();
    let rs = g.resize(xi, 5, 14, ResizeMode::Bilinear).unwrap();
    for o in [mm, conv, sm, ln, att, rs] {
        g.output(o).unwrap();
    }
    let feeds = [&a, &x, &s, &q, &kk, &v].map(Feed::Tensor);
    let (cpu, _) = check("numpy graph", &g, &feeds, 1e-4, 1e-4);
    for (i, name) in ["mm", "conv", "softmax", "ln", "att", "resize"]
        .iter()
        .enumerate()
    {
        let want = load(&dir, name);
        let d = compare(&cpu[i].to_f32(), &want, 1e-5, 1e-5);
        assert!(
            d.worst <= 1.0,
            "{name}: the CPU reference is {} from numpy at {}",
            d.max_abs,
            d.at
        );
        eprintln!("{name}: CPU vs numpy max |error| {:.2e}", d.max_abs);
    }
    std::fs::remove_dir_all(&dir).ok();
}
