//! A CPU renderer for previews and thumbnails: the mesh from the isometric view, fitted,
//! lit by a key light over the viewer's shoulder, depth-buffered, supersampled 2×2, on a
//! transparent background. Rows are rendered in parallel bands.

use rayon::prelude::*;

use crate::builder::{cross, srgb_to_linear, sub};
use crate::{Mesh, Up};

/// A decoded base-colour texture: sRGB RGBA, row-major.
#[derive(Clone, Debug)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Model coordinates to the viewer's: Z up, whatever the file's convention.
pub fn canonical(p: [f32; 3], up: Up) -> [f32; 3] {
    match up {
        Up::Z => p,
        Up::Y => [p[0], -p[2], p[1]],
    }
}

/// The isometric view: right, down and forward (into the screen), Z up.
pub const ISO: [[f32; 3]; 3] = {
    // forward = (-1, 1, -1)/√3: looking from front-right-above (+X right, −Y front).
    let s2 = std::f32::consts::FRAC_1_SQRT_2;
    let s3 = 0.577_350_26;
    let s6 = 0.408_248_3;
    [[s2, s2, 0.0], [s6, -s6, -2.0 * s6], [-s3, s3, -s3]]
};

/// Renders `mesh` to straight sRGB RGBA8 of `width × height`. `textures` are by image
/// index (missing entries draw the material colour alone).
pub fn render(mesh: &Mesh, textures: &[Option<Texture>], width: u32, height: u32) -> Vec<u8> {
    let (w, h) = (width.max(1) as usize, height.max(1) as usize);
    let ss = 2usize;
    let (sw, sh) = (w * ss, h * ss);
    let [right, down, fwd] = ISO;
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let c = canonical(mesh.bounds.centre(), mesh.up);
    // Projected positions: x, y in supersampled pixels, z depth (larger is farther).
    let proj: Vec<[f32; 3]> = mesh
        .positions
        .par_iter()
        .map(|&p| {
            let q = sub(canonical(p, mesh.up), c);
            [dot(q, right), dot(q, down), dot(q, fwd)]
        })
        .collect();
    let (lo, hi) = proj
        .par_iter()
        .fold(
            || ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]),
            |(mut lo, mut hi), p| {
                for k in 0..2 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
                (lo, hi)
            },
        )
        .reduce(
            || ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]),
            |a, b| {
                (
                    [a.0[0].min(b.0[0]), a.0[1].min(b.0[1])],
                    [a.1[0].max(b.1[0]), a.1[1].max(b.1[1])],
                )
            },
        );
    let ext = [(hi[0] - lo[0]).max(1e-20), (hi[1] - lo[1]).max(1e-20)];
    let scale = 0.92 * (sw as f32 / ext[0]).min(sh as f32 / ext[1]);
    let mid = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
    let screen: Vec<[f32; 3]> = proj
        .par_iter()
        .map(|p| {
            [
                (p[0] - mid[0]) * scale + sw as f32 / 2.0,
                (p[1] - mid[1]) * scale + sh as f32 / 2.0,
                p[2],
            ]
        })
        .collect();
    drop(proj);
    // Light: from upper left, towards the scene, in view space; plus a soft fill.
    let light = {
        let l = [0.45f32, 0.55, 0.70];
        let n = dot(l, l).sqrt();
        l.map(|v| v / n)
    };
    let view_normal = |n: [f32; 3]| {
        let n = canonical(n, mesh.up);
        [dot(n, right), dot(n, down), dot(n, fwd)]
    };
    let mats: Vec<[f32; 4]> = mesh.materials.iter().map(|m| m.base_color).collect();
    let tri_material: Vec<(u32, u32, Option<u32>)> = mesh
        .parts
        .iter()
        .map(|p| (p.first, p.first + p.count, p.material))
        .collect();
    let band_rows = 16usize;
    let bands: Vec<(Vec<f32>, Vec<[f32; 4]>)> = (0..sh.div_ceil(band_rows))
        .into_par_iter()
        .map(|band| {
            let y0 = band * band_rows;
            let y1 = (y0 + band_rows).min(sh);
            let rows = y1 - y0;
            let mut depth = vec![f32::INFINITY; rows * sw];
            let mut color = vec![[0f32; 4]; rows * sw];
            for &(first, end, mat) in &tri_material {
                let base = mat
                    .and_then(|m| mats.get(m as usize))
                    .copied()
                    .unwrap_or([0.8, 0.8, 0.8, 1.0]);
                let tex = mat
                    .and_then(|m| mesh.materials.get(m as usize))
                    .and_then(|m| m.texture)
                    .and_then(|t| textures.get(t as usize))
                    .and_then(Option::as_ref);
                for t in first..end {
                    let i = t as usize * 3;
                    let ix = [mesh.indices[i], mesh.indices[i + 1], mesh.indices[i + 2]]
                        .map(|v| v as usize);
                    let [a, b, cc] = ix.map(|v| screen[v]);
                    let ymin = a[1].min(b[1]).min(cc[1]);
                    let ymax = a[1].max(b[1]).max(cc[1]);
                    if ymax < y0 as f32 || ymin >= y1 as f32 {
                        continue;
                    }
                    let area = (b[0] - a[0]) * (cc[1] - a[1]) - (b[1] - a[1]) * (cc[0] - a[0]);
                    if area.abs() < 1e-12 || !area.is_finite() {
                        continue;
                    }
                    let xmin = a[0].min(b[0]).min(cc[0]).max(0.0).floor() as usize;
                    let xmax = (a[0].max(b[0]).max(cc[0]).ceil() as usize).min(sw);
                    let ys = (ymin.max(y0 as f32).floor() as usize).max(y0);
                    let ye = (ymax.ceil() as usize).min(y1);
                    if xmin >= xmax || ys >= ye {
                        continue;
                    }
                    let flat_n = {
                        let p = ix.map(|v| mesh.positions[v]);
                        view_normal(cross(sub(p[1], p[0]), sub(p[2], p[0])))
                    };
                    let normals = ix.map(|v| view_normal(mesh.normals[v]));
                    let cols = mesh.colors.as_ref().map(|c| {
                        ix.map(|v| {
                            let c = c[v];
                            [
                                srgb_to_linear(f32::from(c[0]) / 255.0),
                                srgb_to_linear(f32::from(c[1]) / 255.0),
                                srgb_to_linear(f32::from(c[2]) / 255.0),
                                f32::from(c[3]) / 255.0,
                            ]
                        })
                    });
                    let uvs = mesh.uvs.as_ref().map(|u| ix.map(|v| u[v]));
                    let inv = 1.0 / area;
                    for y in ys..ye {
                        let py = y as f32 + 0.5;
                        for x in xmin..xmax {
                            let px = x as f32 + 0.5;
                            let w0 =
                                ((b[0] - px) * (cc[1] - py) - (b[1] - py) * (cc[0] - px)) * inv;
                            let w1 =
                                ((cc[0] - px) * (a[1] - py) - (cc[1] - py) * (a[0] - px)) * inv;
                            let w2 = 1.0 - w0 - w1;
                            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                                continue;
                            }
                            let z = w0 * a[2] + w1 * b[2] + w2 * cc[2];
                            let k = (y - y0) * sw + x;
                            if z >= depth[k] {
                                continue;
                            }
                            depth[k] = z;
                            let mut n = if mesh.flat {
                                flat_n
                            } else {
                                let m = |c: usize| {
                                    w0 * normals[0][c] + w1 * normals[1][c] + w2 * normals[2][c]
                                };
                                [m(0), m(1), m(2)]
                            };
                            let l = dot(n, n).sqrt().max(1e-20);
                            n = n.map(|v| v / l);
                            // Two-sided: face the viewer.
                            if n[2] > 0.0 {
                                n = n.map(|v| -v);
                            }
                            let lambert = (-dot(n, light)).max(0.0);
                            let shade = 0.22 + 0.78 * lambert;
                            let mut col = base;
                            if let Some(cs) = &cols {
                                for (ch, out) in col.iter_mut().enumerate() {
                                    *out *= w0 * cs[0][ch] + w1 * cs[1][ch] + w2 * cs[2][ch];
                                }
                            }
                            if let (Some(tx), Some(uv)) = (tex, &uvs) {
                                let u = w0 * uv[0][0] + w1 * uv[1][0] + w2 * uv[2][0];
                                let v = w0 * uv[0][1] + w1 * uv[1][1] + w2 * uv[2][1];
                                let s = sample(tx, u, v);
                                for (ch, out) in col.iter_mut().enumerate() {
                                    *out *= s[ch];
                                }
                            }
                            color[k] = [col[0] * shade, col[1] * shade, col[2] * shade, 1.0];
                        }
                    }
                }
            }
            (depth, color)
        })
        .collect();
    // Resolve 2×2 and encode.
    let mut out = vec![0u8; w * h * 4];
    out.par_chunks_exact_mut(w * 4)
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..w {
                let mut acc = [0f32; 4];
                for dy in 0..ss {
                    let sy = y * ss + dy;
                    let (band, ry) = (sy / band_rows, sy % band_rows);
                    let colors = &bands[band].1;
                    for dx in 0..ss {
                        let c = colors[ry * sw + x * ss + dx];
                        // Premultiplied average.
                        for ch in 0..3 {
                            acc[ch] += c[ch] * c[3];
                        }
                        acc[3] += c[3];
                    }
                }
                let n = (ss * ss) as f32;
                let a = acc[3] / n;
                let px = &mut row[x * 4..x * 4 + 4];
                if a > 0.0 {
                    for ch in 0..3 {
                        px[ch] = crate::builder::linear_to_srgb8(acc[ch] / n / a);
                    }
                    px[3] = (a * 255.0 + 0.5) as u8;
                }
            }
        });
    out
}

/// Bilinear-free nearest sample, repeating, to linear RGBA.
fn sample(t: &Texture, u: f32, v: f32) -> [f32; 4] {
    if t.width == 0 || t.height == 0 || t.rgba.len() < (t.width * t.height * 4) as usize {
        return [1.0; 4];
    }
    let fu = u - u.floor();
    let fv = v - v.floor();
    let x = ((fu * t.width as f32) as u32).min(t.width - 1);
    let y = ((fv * t.height as f32) as u32).min(t.height - 1);
    let i = ((y * t.width + x) * 4) as usize;
    let p = &t.rgba[i..i + 4];
    [
        srgb_to_linear(f32::from(p[0]) / 255.0),
        srgb_to_linear(f32::from(p[1]) / 255.0),
        srgb_to_linear(f32::from(p[2]) / 255.0),
        f32::from(p[3]) / 255.0,
    ]
}
