//! The icons of the other apps by the same people, beside their rows in Settings › More
//! from Amazynk.
//!
//! Each is a PNG in the app (`assets/apps/`), scaled once to the size the rows draw it at
//! and handed to the chrome as a texture. The UI renderer blends premultiplied colour, so
//! the scaling averages in linear light with the colour already multiplied by its alpha: an
//! icon's rounded corners then fade into whatever is behind them, with no dark fringe.

use specular_gfx::{Gpu, TexId, UiRenderer, wgpu};
use specular_log::{UI, debug};
use specular_widgets::Chrome;

/// Each row's setting, and its app's icon: a square PNG with straight alpha.
const ICONS: [(&str, &[u8]); 3] = [
    (
        "amazynk.get_zynk",
        include_bytes!("../../assets/apps/zynk.png"),
    ),
    (
        "amazynk.get_pagebara",
        include_bytes!("../../assets/apps/pagebara.png"),
    ),
    (
        "amazynk.get_dozed",
        include_bytes!("../../assets/apps/dozed.png"),
    ),
];

/// The icons as the chrome has them.
#[derive(Default)]
pub(super) struct AppIcons {
    /// The side they were made at (physical px); 0 before they are made.
    side: u32,
    textures: Vec<(wgpu::Texture, TexId)>,
}

impl AppIcons {
    /// Makes the icons `side` physical px wide, unless they are that already, and gives
    /// them to the chrome.
    pub(super) fn prepare(
        &mut self,
        gpu: &Gpu,
        ui: &mut UiRenderer,
        chrome: &mut Chrome,
        side: u32,
    ) {
        let side = side.clamp(8, 512);
        if side == self.side {
            return;
        }
        self.side = side;
        for (_, id) in self.textures.drain(..) {
            ui.unregister_texture(id);
        }
        for (key, png) in ICONS {
            let icon = match specular_png::decode(png, &Default::default()) {
                Ok(icon) => icon,
                Err(e) => {
                    debug!(target: UI, "the icon of {key}: {e}");
                    chrome.set_setting_icon(key, None);
                    continue;
                }
            };
            let px = scaled_premultiplied(&icon.to_rgba8(), icon.width, icon.height, side);
            let (tex, id) = super::sound_page::upload_rgba(gpu, ui, side, side, &px);
            chrome.set_setting_icon(key, Some(id));
            self.textures.push((tex, id));
        }
    }
}

fn to_linear(v: u8) -> f32 {
    let c = f32::from(v) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn to_srgb(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let v = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (v * 255.0).round() as u8
}

/// `rgba` (`w` by `h`, sRGB with straight alpha) as a `side` by `side` picture with
/// premultiplied alpha: each pixel the average of the part of the source it covers.
fn scaled_premultiplied(rgba: &[u8], w: u32, h: u32, side: u32) -> Vec<u8> {
    let (w, h, side) = (w as usize, h as usize, side as usize);
    let mut out = vec![0; side * side * 4];
    if w == 0 || h == 0 || rgba.len() < w * h * 4 {
        return out;
    }
    let (sx, sy) = (w as f32 / side as f32, h as f32 / side as f32);
    // The source pixels a span of the output covers, each with the share of it covered.
    let span = |i: usize, step: f32, n: usize| {
        let (a, b) = (i as f32 * step, (i + 1) as f32 * step);
        let last = (b.ceil() as usize).clamp(1, n);
        ((a.floor() as usize).min(n - 1)..last).map(move |s| {
            let cover = (b.min((s + 1) as f32) - a.max(s as f32)).max(0.0);
            (s, cover)
        })
    };
    for y in 0..side {
        for x in 0..side {
            let (mut sum, mut weight) = ([0.0f32; 4], 0.0f32);
            for (v, wy) in span(y, sy, h) {
                for (u, wx) in span(x, sx, w) {
                    let p = &rgba[(v * w + u) * 4..][..4];
                    let (wgt, a) = (wx * wy, f32::from(p[3]) / 255.0);
                    for c in 0..3 {
                        sum[c] += to_linear(p[c]) * a * wgt;
                    }
                    sum[3] += a * wgt;
                    weight += wgt;
                }
            }
            if weight > 0.0 {
                let o = &mut out[(y * side + x) * 4..][..4];
                for c in 0..3 {
                    o[c] = to_srgb(sum[c] / weight);
                }
                o[3] = (sum[3] / weight * 255.0).round() as u8;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every icon in the app decodes, is square, and has transparent corners (it brings
    /// its own shape) and an opaque middle.
    #[test]
    fn the_icons_are_square_pictures_with_their_own_shape() {
        for (key, png) in ICONS {
            let icon = specular_png::decode(png, &Default::default()).expect(key);
            assert_eq!(icon.width, icon.height, "{key}");
            let (side, px) = (icon.width as usize, icon.to_rgba8());
            assert_eq!(px[3], 0, "{key}: the corner");
            assert_eq!(
                px[(side / 2 * side + side / 2) * 4 + 3],
                255,
                "{key}: the middle"
            );
            let small = scaled_premultiplied(&px, icon.width, icon.height, 36);
            assert_eq!(small.len(), 36 * 36 * 4);
            assert_eq!(small[3], 0, "{key}: the corner, scaled");
            assert_eq!(
                small[(18 * 36 + 18) * 4 + 3],
                255,
                "{key}: the middle, scaled"
            );
        }
    }

    /// Opaque colour keeps its value at any size; a pixel half over nothing has half the
    /// alpha and half the light, not a dark edge.
    #[test]
    fn scaling_averages_premultiplied_light() {
        let red = [200u8, 40, 10, 255];
        let flat: Vec<u8> = red.iter().copied().cycle().take(4 * 4 * 4).collect();
        for side in [1, 2, 3, 8] {
            let out = scaled_premultiplied(&flat, 4, 4, side);
            assert!(out.chunks_exact(4).all(|p| p == red), "{side}: {out:?}");
        }
        // White beside nothing, halved: half alpha, and white light times a half.
        let pair = [255u8, 255, 255, 255, 0, 0, 0, 0];
        let out = scaled_premultiplied(&pair, 2, 1, 1);
        assert_eq!(out[3], 128);
        assert_eq!(out[0], to_srgb(0.5));
        assert_eq!(scaled_premultiplied(&[], 0, 0, 2), vec![0; 16]);
    }
}
