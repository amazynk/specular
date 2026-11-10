//! The sound page's colours from its cover: the page chrome (canvas, panels, lines, the
//! choices' accent) tinted with the cover's dominant colours, and the scrim that keeps text
//! readable over the blurred cover behind the page.
//!
//! - Dominant colours: k-means (k = 5, deterministic start at luminance quantiles) on at most
//!   32 × 32 samples of the cover.
//! - Tint: the most common colour, matched to each chrome colour's luminance and mixed in;
//!   the most colourful common colour becomes the accent. The mix is the strongest of a few
//!   steps whose every text pair keeps WCAG AA (4.5:1 text, 3:1 lines and marks) — or the
//!   theme's own contrast, where that is lower — else the theme stays as it is.
//! - Data colours (waveform, played part, spectrogram ramp, meters) never change here.
//! - Backdrop: the scrim (the canvas colour at an alpha) is the least alpha from 0.55 up that
//!   keeps the same pairs over every sample of the blurred cover.

use specular_theme::{Color, Mode, Palette};

/// WCAG thresholds: text, and lines and marks (non-text).
const AA_TEXT: f32 = 4.5;
const AA_MARK: f32 = 3.0;

fn lin(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn lum3(l: [f32; 3]) -> f32 {
    0.2126 * l[0] + 0.7152 * l[1] + 0.0722 * l[2]
}

/// WCAG contrast of two relative luminances.
pub(super) fn contrast_l(a: f32, b: f32) -> f32 {
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// WCAG contrast of two colours, `top` composited over opaque `under` by its alpha
/// (linear light, as the GPU blends).
pub(super) fn contrast(top: Color, under: Color) -> f32 {
    let t = over(top, [lin(under.r), lin(under.g), lin(under.b)]);
    contrast_l(lum3(t), under.luminance())
}

/// `c` over linear `under`, in linear light.
fn over(c: Color, under: [f32; 3]) -> [f32; 3] {
    let a = c.a;
    [
        lin(c.r) * a + under[0] * (1.0 - a),
        lin(c.g) * a + under[1] * (1.0 - a),
        lin(c.b) * a + under[2] * (1.0 - a),
    ]
}

/// The cover's dominant colours, most common first, with their shares (0…1).
pub(super) fn dominant(w: u32, h: u32, rgba: &[u8]) -> Vec<(Color, f32)> {
    let px = samples(w, h, rgba, 32);
    if px.is_empty() {
        return Vec::new();
    }
    let mut sorted = px.clone();
    sorted.sort_by(|a, b| lum3(*a).total_cmp(&lum3(*b)));
    let k = 5.min(px.len());
    let mut centres: Vec<[f32; 3]> = (0..k)
        .map(|i| sorted[((2 * i + 1) * sorted.len()) / (2 * k)])
        .collect();
    let mut counts = vec![0usize; k];
    for _ in 0..10 {
        let mut sums = vec![[0.0f32; 3]; k];
        counts.fill(0);
        for p in &px {
            let j = nearest(&centres, *p);
            for c in 0..3 {
                sums[j][c] += p[c];
            }
            counts[j] += 1;
        }
        for j in 0..k {
            if counts[j] > 0 {
                centres[j] = sums[j].map(|s| s / counts[j] as f32);
            }
        }
    }
    let n = px.len() as f32;
    let mut out: Vec<(Color, f32)> = centres
        .iter()
        .zip(&counts)
        .filter(|(_, n)| **n > 0)
        .map(|(c, &m)| (Color::rgba(c[0], c[1], c[2], 1.0), m as f32 / n))
        .collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1));
    out
}

fn nearest(centres: &[[f32; 3]], p: [f32; 3]) -> usize {
    let d = |c: &[f32; 3]| (0..3).map(|i| (c[i] - p[i]).powi(2)).sum::<f32>();
    (0..centres.len())
        .min_by(|&a, &b| d(&centres[a]).total_cmp(&d(&centres[b])))
        .unwrap_or(0)
}

/// The cover averaged down to at most `n` × `n` cells (sRGB 0…1): its colours for the
/// k-means, and the blurred backdrop's range for the scrim.
pub(super) fn samples(w: u32, h: u32, rgba: &[u8], n: u32) -> Vec<[f32; 3]> {
    let (w, h) = (w as usize, h as usize);
    if w == 0 || h == 0 || rgba.len() < w * h * 4 {
        return Vec::new();
    }
    let (cw, ch) = (w.min(n as usize), h.min(n as usize));
    let mut out = Vec::with_capacity(cw * ch);
    for cy in 0..ch {
        let (y0, y1) = (cy * h / ch, ((cy + 1) * h / ch).max(cy * h / ch + 1));
        for cx in 0..cw {
            let (x0, x1) = (cx * w / cw, ((cx + 1) * w / cw).max(cx * w / cw + 1));
            let mut s = [0.0f32; 3];
            let mut m = 0.0f32;
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = &rgba[(y * w + x) * 4..];
                    let a = f32::from(p[3]) / 255.0;
                    for c in 0..3 {
                        s[c] += f32::from(p[c]) / 255.0 * a;
                    }
                    m += a;
                }
            }
            if m > 0.0 {
                out.push(s.map(|v| v / m));
            }
        }
    }
    out
}

/// `c` with its luminance moved to `target` (mixed towards black or white).
fn at_luminance(c: Color, target: f32) -> Color {
    let towards = if c.luminance() > target {
        Color::BLACK
    } else {
        Color::WHITE
    };
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..20 {
        let mid = (lo + hi) / 2.0;
        let l = c.lerp(towards, mid).luminance();
        if (l > target) == (towards == Color::BLACK) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    c.lerp(towards, hi).with_alpha(1.0)
}

fn chroma(c: Color) -> f32 {
    c.r.max(c.g).max(c.b) - c.r.min(c.g).min(c.b)
}

/// The pairs every themed palette keeps: (foreground, background, threshold).
fn pairs(p: &Palette) -> [(Color, Color, f32); 8] {
    let soft_on_surface = {
        let l = over(
            p.accent_soft,
            [lin(p.surface.r), lin(p.surface.g), lin(p.surface.b)],
        );
        // Back to sRGB for the pair.
        let s = |v: f32| {
            if v <= 0.003_130_8 {
                v * 12.92
            } else {
                1.055 * v.powf(1.0 / 2.4) - 0.055
            }
        };
        Color::rgba(s(l[0]), s(l[1]), s(l[2]), 1.0)
    };
    [
        (p.text, p.canvas, AA_TEXT),
        (p.text, p.surface, AA_TEXT),
        (p.text_muted, p.canvas, AA_TEXT),
        (p.text_muted, p.surface, AA_TEXT),
        (p.text_faint, p.canvas, AA_MARK),
        (p.text_faint, p.surface, AA_MARK),
        (p.accent, p.surface, AA_MARK),
        (p.text, soft_on_surface, AA_TEXT),
    ]
}

/// Whether `p` keeps every pair at AA, or at `base`'s own contrast where that is lower.
pub(super) fn readable(base: &Palette, p: &Palette) -> bool {
    pairs(base)
        .iter()
        .zip(pairs(p).iter())
        .all(|(b, t)| contrast(t.0, t.1) >= contrast(b.0, b.1).min(t.2) - 1e-3)
}

/// The theme's page colours tinted by the cover's `colours` ([`dominant`]); the theme as it
/// is when no tint keeps text readable.
pub(super) fn themed(base: &Palette, colours: &[(Color, f32)]) -> Palette {
    let Some(&(tint, _)) = colours.first() else {
        return base.clone();
    };
    let accent = colours
        .iter()
        .filter(|(c, share)| chroma(*c) > 0.2 && *share > 0.04)
        .max_by(|a, b| (chroma(a.0) * a.1.sqrt()).total_cmp(&(chroma(b.0) * b.1.sqrt())))
        .map(|c| c.0);
    for k in [0.7f32, 0.55, 0.45, 0.35, 0.25, 0.15, 0.08] {
        let mix = |c: Color| c.lerp(at_luminance(tint, c.luminance()), k).with_alpha(c.a);
        let mut p = base.clone();
        p.canvas = mix(base.canvas);
        p.surface = mix(base.surface);
        p.surface_raised = mix(base.surface_raised);
        p.separator = mix(base.separator);
        if let Some(a) = accent {
            // Lightened (dark theme) or darkened until it stands out from the panels.
            let towards = if base.mode == Mode::Dark {
                Color::WHITE
            } else {
                Color::BLACK
            };
            let mut c = a;
            for _ in 0..10 {
                if contrast(c, p.surface) >= AA_MARK {
                    break;
                }
                c = c.lerp(towards, 0.15);
            }
            p.accent = c.with_alpha(1.0);
            p.accent_soft = c.with_alpha(base.accent_soft.a);
        }
        if readable(base, &p) {
            return p;
        }
        // Without the accent, then.
        p.accent = base.accent;
        p.accent_soft = base.accent_soft;
        if readable(base, &p) {
            return p;
        }
    }
    base.clone()
}

/// The lowest contrast of `fg` over the canvas at alpha `a` over each backdrop sample.
fn worst(fg: Color, canvas: Color, a: f32, backdrop: &[[f32; 3]]) -> f32 {
    let c = [lin(canvas.r), lin(canvas.g), lin(canvas.b)];
    backdrop
        .iter()
        .map(|s| {
            let under = [0, 1, 2].map(|i| lin(s[i]) * (1.0 - a) + c[i] * a);
            contrast_l(lum3(over(fg, under)), lum3(under))
        })
        .fold(f32::INFINITY, f32::min)
}

/// Text over the blurred cover: the scrim (the canvas colour at an alpha, from
/// [`SCRIM`] up) and the text colours, moved away from the backdrop (lighter on a dark
/// theme, darker on a light one) until text and muted text keep 4.5:1 and faint marks 3:1
/// over every sample of the cover (`backdrop`, from [`samples`]). The palette `p` with
/// those text colours, and the alpha.
pub(super) fn over_backdrop(p: &Palette, backdrop: &[[f32; 3]]) -> (Palette, f32) {
    let towards = if p.mode == Mode::Dark {
        Color::WHITE
    } else {
        Color::BLACK
    };
    let fit = |fg: Color, th: f32, a: f32| -> Option<Color> {
        (0..=20)
            .map(|k| fg.lerp(towards, k as f32 * 0.05))
            .find(|c| worst(*c, p.canvas, a, backdrop) >= th)
    };
    let mut a = SCRIM;
    while a < 1.0 {
        if let (Some(text), Some(muted), Some(faint)) = (
            fit(p.text, AA_TEXT, a),
            fit(p.text_muted, AA_TEXT, a),
            fit(p.text_faint, AA_MARK, a),
        ) {
            let mut out = p.clone();
            out.text = text;
            out.text_muted = muted;
            out.text_faint = faint;
            return (out, a);
        }
        a += 0.05;
    }
    (p.clone(), 1.0)
}

/// The least scrim: the cover shows through a little over a quarter.
pub(super) const SCRIM: f32 = 0.72;

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
        let mut v = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let c = f(x, y);
                v.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
        v
    }

    fn covers() -> Vec<(&'static str, Vec<u8>)> {
        vec![
            ("red", solid(64, 64, |_, _| [220, 20, 30])),
            ("white", solid(64, 64, |_, _| [255, 255, 255])),
            ("black", solid(64, 64, |_, _| [0, 0, 0])),
            ("yellow", solid(64, 64, |_, _| [250, 230, 40])),
            (
                "gradient",
                solid(64, 64, |x, y| [(x * 4) as u8, (y * 4) as u8, 150]),
            ),
            (
                "halves",
                solid(64, 64, |x, _| {
                    if x < 40 {
                        [20, 60, 160]
                    } else {
                        [240, 200, 60]
                    }
                }),
            ),
            (
                "checks",
                solid(64, 64, |x, y| {
                    if (x / 8 + y / 8) % 2 == 0 {
                        [255, 255, 255]
                    } else {
                        [0, 0, 0]
                    }
                }),
            ),
        ]
    }

    #[test]
    fn dominant_colours_by_share() {
        let px = solid(64, 64, |x, _| {
            if x < 40 {
                [20, 60, 160]
            } else {
                [240, 200, 60]
            }
        });
        let d = dominant(64, 64, &px);
        let (c, share) = d[0];
        assert!((share - 40.0 / 64.0).abs() < 0.05, "{d:?}");
        assert!((c.b - 160.0 / 255.0).abs() < 0.02, "{c:?}");
        assert!(d.iter().map(|x| x.1).sum::<f32>() > 0.999);
        assert!(dominant(0, 0, &[]).is_empty());
    }

    /// Every cover, both themes: the themed page keeps AA (or the theme's own contrast),
    /// and so does the canvas over the blurred cover at the chosen scrim.
    #[test]
    fn themed_colours_keep_contrast() {
        for base in [Palette::dark(), Palette::light()] {
            for (name, px) in covers() {
                let d = dominant(64, 64, &px);
                let p = themed(&base, &d);
                assert!(readable(&base, &p), "{name} {:?}", base.mode);
                for ((fg, bg, th), (bf, bb, _)) in pairs(&p).into_iter().zip(pairs(&base)) {
                    let need = contrast(bf, bb).min(th);
                    assert!(contrast(fg, bg) >= need - 1e-3, "{name}");
                }
                // Over the blurred cover: AA over every sample at the scrim chosen, and the
                // panels' text still readable with the text colours it moved.
                let s = samples(64, 64, &px, 16);
                let (q, a) = over_backdrop(&p, &s);
                assert!((SCRIM..=1.0).contains(&a), "{name}: {a}");
                assert!(worst(q.text, q.canvas, a, &s) >= AA_TEXT - 1e-3, "{name}");
                assert!(
                    worst(q.text_muted, q.canvas, a, &s) >= AA_TEXT - 1e-3,
                    "{name}"
                );
                assert!(
                    worst(q.text_faint, q.canvas, a, &s) >= AA_MARK - 1e-3,
                    "{name}"
                );
                assert!(readable(&base, &q), "{name}: panels");
            }
        }
    }

    #[test]
    fn colourful_covers_tint_and_grey_ones_keep_the_accent() {
        let base = Palette::dark();
        let red = solid(32, 32, |_, _| [200, 30, 40]);
        let p = themed(&base, &dominant(32, 32, &red));
        assert_ne!(p.canvas, base.canvas);
        assert!(p.canvas.r > p.canvas.b, "{:?}", p.canvas);
        assert_ne!(p.accent, base.accent);
        let grey = solid(32, 32, |_, _| [128, 128, 128]);
        let g = themed(&base, &dominant(32, 32, &grey));
        assert_eq!(g.accent, base.accent);
        // No cover colours: the theme.
        assert_eq!(themed(&base, &[]).canvas, base.canvas);
    }
}
