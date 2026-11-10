//! The performance HUD (F12): scale, threads, decode and upload times, frame time.

use specular_gfx::Rect;
use specular_i18n::{ByteUnits, I18n, t};
use specular_theme::metrics;

use crate::HudStats;
use crate::ctx::{Ctx, HAlign};

/// Draws the panel at the top leading corner, `inset` in from the side and `top` down.
pub(crate) fn draw(cx: &mut Ctx, s: &HudStats, inset: f32, top: f32) {
    let loc = I18n::global().localizer();
    let ms = |v: f64| t!("unit-milliseconds", value = (v * 10.0).round() / 10.0);
    let level = match s.level {
        0 => t!("app-hud-level-thumbnail"),
        1 => t!("app-hud-level-full"),
        n => t!("app-hud-level-scaled", denominator = n as i64),
    };
    let rows: Vec<(String, String)> = vec![
        (t!("app-hud-scale"), loc.percent(s.scale as f64, 1)),
        (t!("app-hud-level"), level),
        (t!("app-hud-open"), ms(s.open_ms)),
        (t!("app-hud-decode"), ms(s.decode_ms)),
        (t!("app-hud-upload"), ms(s.upload_ms)),
        (t!("app-hud-frame"), ms(s.frame_ms)),
        (
            t!("app-hud-threads"),
            t!(
                "app-hud-threads-value",
                threads = s.threads as i64,
                queued = s.queued as i64
            ),
        ),
        (
            t!("app-hud-tiles"),
            t!(
                "app-hud-tiles-value",
                tiles = s.tiles as i64,
                capacity = s.tile_capacity as i64
            ),
        ),
        (t!("app-hud-ram"), loc.bytes(s.ram_bytes, ByteUnits::Binary)),
        (
            t!("app-hud-vram"),
            loc.bytes(s.vram_bytes, ByteUnits::Binary),
        ),
        (t!("app-hud-gpu"), s.backend.clone()),
    ];
    let ls = cx.style(metrics::FONT_MONO);
    let vs = cx.style(metrics::FONT_MONO).tabular().weight(550.0);
    let lw = rows
        .iter()
        .map(|(l, _)| cx.layout(l, ls, None).width)
        .fold(0.0f32, f32::max);
    let vw = rows
        .iter()
        .map(|(_, v)| cx.layout(v, vs, None).width)
        .fold(70.0f32, f32::max);
    let line = 17.0 * cx.style(1.0).size;
    let w = lw + vw + 36.0;
    let h = rows.len() as f32 * line + 16.0;
    let r = cx.m(Rect::new(14.0 + inset, top, w, h));
    cx.dl
        .rect(r, metrics::RADIUS_MD, cx.pal.tooltip_bg.with_alpha(0.78));
    for (i, (l, v)) in rows.iter().enumerate() {
        let y = r.y + 8.0 + i as f32 * line;
        let lr = cx.m_in(Rect::new(r.x + 12.0, y, lw, line), r);
        cx.label(l, ls, lr, HAlign::Start, cx.pal.tooltip_text.fade(0.7));
        let vr = cx.m_in(Rect::new(r.right() - 12.0 - vw, y, vw, line), r);
        cx.label(v, vs, vr, HAlign::End, cx.pal.hud_text);
    }
}
