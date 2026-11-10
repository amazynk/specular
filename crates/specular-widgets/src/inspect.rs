//! Inspection panels (DESIGN.md §6.2): the histogram of the visible image and the pixel
//! inspector (position, source samples, sRGB and hex, linear light, with a magnified loupe).
//!
//! The panels stack in the bottom corner on the start side, clear of the toolbar. The
//! loupe is a hole in the inspector panel: the application renders the magnified image there
//! (in the scene, under the chrome) at [`Chrome::pixel_loupe`]. Values come from the GPU probe
//! and the decoder; the histogram from the GPU compute pass. The histogram's tone axis runs
//! dark to light left to right in every language (it is data, like the image), while the
//! panels themselves mirror.
//!
//! The colour-vision-safe display aids live here too (DESIGN.md §6.2): false colour with its
//! colour bar (values in the image's own units, the gradient low to high left to right like
//! the histogram's axis, the panel mirrored), channel-colour presets, HiLo and the
//! colour-vision simulation, each a transient toggle that starts from its setting.

use specular_gfx::Rect;
use specular_i18n::{I18n, t};
use specular_inspect::{
    Channel, ChannelColours, ChannelView, ColourMap, Cvd, DisplayValue, Histogram, Legend,
    SourceValue, hex, tick_step, ticks,
};
use specular_settings::{
    ChannelColourPreset, ColourVision, FalseColourMap, HistogramChannels, Settings,
};
use specular_theme::{Color, metrics};

use crate::Chrome;
use crate::ctx::{Ctx, HAlign};

/// Which inspection tools are on (each toggled by its action).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InspectTools {
    /// Highlight / shadow clipping overlay (`j`).
    pub clipping: bool,
    /// Focus peaking overlay (`Shift+F`).
    pub peaking: bool,
    /// Histogram panel (`Shift+G`).
    pub histogram: bool,
    /// Pixel inspector panel and loupe (`Shift+I`).
    pub pixel: bool,
    /// Channel isolation (`Shift+1…5`).
    pub channel: ChannelView,
    /// False colour through `colour_map`, with its colour bar (`Shift+C`).
    pub false_colour: bool,
    /// The false-colour map (`Shift+B` steps it); starts from the setting.
    pub colour_map: ColourMap,
    /// How the colour channels are coloured; starts from the setting.
    pub channel_colours: ChannelColours,
    /// HiLo: pixels at the bit depth's maximum and minimum marked (`Shift+H`).
    pub hilo: bool,
    /// Colour-vision simulation of the whole window (`Shift+V`); starts from the setting.
    pub cvd: Cvd,
}

/// The decoded value of the inspected pixel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SourceState {
    /// The format or layout cannot be sampled.
    #[default]
    Unavailable,
    /// Being read from the decoder.
    Pending,
    Value(SourceValue),
}

/// The pixel under the pointer, in stored (file) coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PixelInfo {
    pub x: u32,
    pub y: u32,
    pub source: SourceState,
    /// The displayed colour (from the GPU probe; `None` until it arrives).
    pub display: Option<DisplayValue>,
}

/// The inspection tools' state and data, owned by the chrome; the application fills in the
/// histogram and the pixel as its readbacks arrive.
#[derive(Default)]
pub struct Inspect {
    pub tools: InspectTools,
    pub histogram: Option<Box<Histogram>>,
    /// `None` while the pointer is not over the image.
    pub pixel: Option<PixelInfo>,
    /// What the colour bar shows (map, range in source units), set by the application for
    /// the image on screen; the bar is drawn while false colour is on.
    pub legend: Option<Legend>,
    /// The top edge (logical px) of panels the application draws in the same bottom start
    /// corner this frame (a science image's plane sliders and scale bar): the inspection
    /// panels stack above it instead of over them.
    pub floor: Option<f32>,
    /// Where the loupe goes (logical px), from the last draw.
    pub(crate) loupe: Option<Rect>,
    /// The display-aid settings last applied to `tools`.
    synced: Option<(FalseColourMap, ChannelColourPreset, ColourVision)>,
}

impl Inspect {
    /// Applies the display-aid settings (map, channel colours, colour-vision simulation) to the
    /// tools when they change (and the first time): the keys change the tools for the session,
    /// Settings changes them again.
    pub fn sync_settings(&mut self, settings: &Settings) {
        let i = &settings.inspection;
        let now = (i.false_colour_map, i.channel_colours, i.colour_vision);
        let last = self.synced.replace(now);
        if last.map(|l| l.0) != Some(now.0) {
            self.tools.colour_map = colour_map(now.0);
        }
        if last.map(|l| l.1) != Some(now.1) {
            self.tools.channel_colours = channel_colours(now.1);
        }
        if last.map(|l| l.2) != Some(now.2) {
            self.tools.cvd = colour_vision(now.2);
        }
    }

    /// The colour map false colour uses now, if it is on.
    pub fn false_colour(&self) -> Option<ColourMap> {
        self.tools.false_colour.then_some(self.tools.colour_map)
    }
}

/// The colour map a setting names.
pub fn colour_map(m: FalseColourMap) -> ColourMap {
    ColourMap::from_id(&m.to_string()).unwrap_or_default()
}

/// The channel colours a setting names.
pub fn channel_colours(c: ChannelColourPreset) -> ChannelColours {
    match c {
        ChannelColourPreset::RedGreen => ChannelColours::RedGreen,
        ChannelColourPreset::MagentaGreen => ChannelColours::MagentaGreen,
        ChannelColourPreset::MagentaYellowCyan => ChannelColours::MagentaYellowCyan,
    }
}

/// The simulation a setting names.
pub fn colour_vision(c: ColourVision) -> Cvd {
    Cvd::from_id(&c.to_string()).unwrap_or_default()
}

/// A colour map's (translatable) name.
pub fn colour_map_name(m: ColourMap) -> String {
    match m {
        ColourMap::Viridis => t!("colour-map-viridis"),
        ColourMap::Cividis => t!("colour-map-cividis"),
        ColourMap::Batlow => t!("colour-map-batlow"),
        ColourMap::Roma => t!("colour-map-roma"),
        ColourMap::Vik => t!("colour-map-vik"),
        ColourMap::Oslo => t!("colour-map-oslo"),
        ColourMap::Lajolla => t!("colour-map-lajolla"),
        ColourMap::Thermal => t!("colour-map-thermal"),
        ColourMap::Haline => t!("colour-map-haline"),
        ColourMap::Solar => t!("colour-map-solar"),
        ColourMap::Ice => t!("colour-map-ice"),
        ColourMap::Deep => t!("colour-map-deep"),
        ColourMap::Dense => t!("colour-map-dense"),
        ColourMap::Algae => t!("colour-map-algae"),
        ColourMap::Rain => t!("colour-map-rain"),
        ColourMap::Speed => t!("colour-map-speed"),
        ColourMap::Balance => t!("colour-map-balance"),
        ColourMap::Curl => t!("colour-map-curl"),
        ColourMap::Phase => t!("colour-map-phase"),
        ColourMap::Jet => t!("colour-map-jet"),
    }
}

/// What a channel-colour preset is called (announced when it changes).
pub fn channel_colours_label(c: ChannelColours) -> String {
    match c {
        ChannelColours::RedGreen => t!("channel-colours-red-green"),
        ChannelColours::MagentaGreen => t!("channel-colours-magenta-green"),
        ChannelColours::MagentaYellowCyan => t!("channel-colours-magenta-yellow-cyan"),
    }
}

/// The badge of a colour-vision simulation (`None` when off).
pub fn cvd_label(c: Cvd) -> Option<String> {
    Some(match c {
        Cvd::Off => return None,
        Cvd::Protanopia => t!("badge-colour-vision-protanopia"),
        Cvd::Deuteranopia => t!("badge-colour-vision-deuteranopia"),
        Cvd::Tritanopia => t!("badge-colour-vision-tritanopia"),
        Cvd::Achromatopsia => t!("badge-colour-vision-achromatopsia"),
    })
}

/// The colour bar's unit: the legend's, or "raw values".
fn legend_unit(l: &Legend) -> String {
    if l.unit.is_empty() {
        t!("colour-bar-unit-raw")
    } else {
        l.unit.clone()
    }
}

/// The colour bar's title: map name and unit.
pub(crate) fn colour_bar_title(l: &Legend) -> String {
    t!(
        "colour-bar-title-unit",
        map = colour_map_name(l.map),
        unit = legend_unit(l)
    )
}

/// Tick values of a colour bar with their localised labels.
pub(crate) fn colour_bar_ticks(l: &Legend, max: usize) -> Vec<(f64, String)> {
    let loc = I18n::global().localizer();
    let step = tick_step(l.lo.min(l.hi), l.lo.max(l.hi), max);
    let digits = if step <= 0.0 || step >= 1.0 {
        0
    } else {
        (-step.log10().floor()).clamp(0.0, 6.0) as u8
    };
    ticks(l.lo, l.hi, max)
        .into_iter()
        // A true minus sign, as the readouts write it, not the hyphen the number format gives.
        .map(|v| (v, loc.number_fixed(v, digits).replace('-', "\u{2212}")))
        .collect()
}

/// The screen-reader summary of the colour bar.
pub(crate) fn colour_bar_summary(l: &Legend) -> String {
    let loc = I18n::global().localizer();
    let digits = if (l.hi - l.lo).abs() >= 10.0 { 0 } else { 3 };
    t!(
        "colour-bar-a11y",
        map = colour_map_name(l.map),
        low = loc.number(l.lo, digits),
        high = loc.number(l.hi, digits),
        unit = legend_unit(l)
    )
}

/// Whether the colour bar is on screen.
pub(crate) fn colour_bar_shown(ins: &Inspect, settings: &Settings) -> bool {
    ins.tools.false_colour && settings.inspection.colour_bar && ins.legend.is_some()
}

/// Graph size (logical px): one column per two bins.
const GRAPH_W: f32 = 256.0;
const GRAPH_H: f32 = 88.0;
const COLUMNS: usize = 128;
/// Loupe side (logical px).
pub const LOUPE: f32 = 112.0;
const PAD: f32 = metrics::PAD;
/// Clearance above the window bottom for the toolbar.
const BOTTOM: f32 = metrics::TOOLBAR_HEIGHT + 2.0 * metrics::TOOLBAR_MARGIN + 6.0;

/// The curves `channels` shows, back to front.
pub(crate) fn channels(c: HistogramChannels) -> &'static [Channel] {
    match c {
        HistogramChannels::Colour => &Channel::RGB,
        HistogramChannels::Luminance => &[Channel::Luma],
        HistogramChannels::Both => &Channel::ALL,
    }
}

fn curve_color(c: Channel, pal: &specular_theme::Palette) -> Color {
    match c {
        Channel::Red => Color::rgba(1.0, 0.27, 0.27, 0.6),
        Channel::Green => Color::rgba(0.25, 0.85, 0.35, 0.6),
        Channel::Blue => Color::rgba(0.3, 0.5, 1.0, 0.6),
        Channel::Luma => pal.tooltip_text.fade(0.42),
    }
}

/// "Clipped: highlights 1.2%, shadows 0%".
pub(crate) fn clipped_text(h: &Histogram) -> String {
    let loc = I18n::global().localizer();
    t!(
        "histogram-clipped",
        high = loc.percent(h.high_fraction() as f64, 1),
        low = loc.percent(h.low_fraction() as f64, 1)
    )
}

/// What the channel badge and its announcement say.
pub fn channel_label(v: ChannelView) -> String {
    match v {
        ChannelView::Normal => t!("channel-normal"),
        ChannelView::Red => t!("channel-red"),
        ChannelView::Green => t!("channel-green"),
        ChannelView::Blue => t!("channel-blue"),
        ChannelView::Alpha => t!("channel-alpha"),
        ChannelView::AlphaChecker => t!("channel-alpha-checker"),
        ChannelView::AlphaMatte => t!("channel-alpha-matte"),
        ChannelView::Luma => t!("channel-luma"),
    }
}

/// The swatch in the channel badge: the channel's colour, white for alpha and luminance.
pub(crate) fn channel_swatch(v: ChannelView) -> Color {
    match v {
        ChannelView::Red => Color::hex(0xEB423D),
        ChannelView::Green => Color::hex(0x4DC75C),
        ChannelView::Blue => Color::hex(0x427AF5),
        ChannelView::Luma => Color::hex(0x9E9E9E),
        _ => Color::WHITE,
    }
}

/// The screen-reader summary of the histogram.
pub(crate) fn histogram_summary(h: &Histogram) -> String {
    let loc = I18n::global().localizer();
    t!(
        "histogram-a11y",
        mean = loc.number(h.stats(Channel::Luma).mean as f64, 0),
        high = loc.percent(h.high_fraction() as f64, 1),
        low = loc.percent(h.low_fraction() as f64, 1)
    )
}

fn join<T: ToString>(v: &[T]) -> String {
    v.iter().map(T::to_string).collect::<Vec<_>>().join(" ")
}

/// The source row: label (names the sample model) and value; `None` when unavailable.
pub(crate) fn source_row(s: &SourceState) -> Option<(String, String)> {
    Some(match s {
        SourceState::Unavailable => return None,
        SourceState::Pending => (t!("inspector-source-pending"), "…".to_owned()),
        SourceState::Value(SourceValue::Gray(v)) => (t!("inspector-source-gray"), v.to_string()),
        SourceState::Value(SourceValue::Rgb(v)) => (t!("inspector-source-rgb"), join(v)),
        SourceState::Value(SourceValue::Rgba(v)) => (t!("inspector-source-rgba"), join(v)),
        SourceState::Value(SourceValue::YCbCr { ycc, .. }) => {
            (t!("inspector-source-ycc"), join(ycc))
        }
        SourceState::Value(SourceValue::Gray16(v)) => {
            (t!("inspector-source-gray16"), v.to_string())
        }
        SourceState::Value(SourceValue::GrayAlpha16(v)) => {
            (t!("inspector-source-gray-alpha16"), join(v))
        }
        // Alpha only when not opaque, as for the sRGB row (most 16-bit RGB sources have none).
        SourceState::Value(SourceValue::Rgba16(v)) if v[3] == u16::MAX => {
            (t!("inspector-source-rgb16"), join(&v[..3]))
        }
        SourceState::Value(SourceValue::Rgba16(v)) => (t!("inspector-source-rgba16"), join(v)),
    })
}

/// The sRGB value as shown: three channels, alpha added when not opaque.
pub(crate) fn srgb_text(d: &DisplayValue) -> String {
    if d.srgb[3] == 255 {
        join(&d.srgb[..3])
    } else {
        join(&d.srgb)
    }
}

/// Label / value rows of the inspector, top to bottom.
pub(crate) fn pixel_rows(p: &PixelInfo) -> Vec<(String, String)> {
    let mut rows = vec![(t!("inspector-position"), format!("{}, {}", p.x, p.y))];
    rows.extend(source_row(&p.source));
    if let Some(d) = &p.display {
        rows.push((t!("inspector-srgb"), srgb_text(d)));
        rows.push((t!("inspector-hex"), hex([d.srgb[0], d.srgb[1], d.srgb[2]])));
        let l = d.linear;
        let lin = if d.srgb[3] == 255 {
            format!("{:.4} {:.4} {:.4}", l[0], l[1], l[2])
        } else {
            format!("{:.4} {:.4} {:.4} {:.3}", l[0], l[1], l[2], l[3])
        };
        rows.push((t!("inspector-linear"), lin));
    }
    rows
}

/// Two pixels' rows side by side (compare's probe): label, the first's value, the
/// second's; a row only one of them has shows "–" for the other.
pub fn pixel_columns(
    a: Option<&PixelInfo>,
    b: Option<&PixelInfo>,
) -> Vec<(String, String, String)> {
    let ra = a.map(pixel_rows).unwrap_or_default();
    let rb = b.map(pixel_rows).unwrap_or_default();
    let mut labels: Vec<String> = ra.iter().map(|r| r.0.clone()).collect();
    for (l, _) in &rb {
        if !labels.contains(l) {
            labels.push(l.clone());
        }
    }
    let find = |rows: &[(String, String)], l: &str| {
        rows.iter()
            .find(|r| r.0 == l)
            .map_or_else(|| "–".to_owned(), |r| r.1.clone())
    };
    labels
        .into_iter()
        .map(|l| {
            let (va, vb) = (find(&ra, &l), find(&rb, &l));
            (l, va, vb)
        })
        .collect()
}

/// One pixel's screen-reader summary ([`pixel_summary`]), for compare's probe.
pub fn pixel_text(p: &PixelInfo) -> String {
    pixel_summary(p)
}

/// The screen-reader summary of the inspected pixel.
pub(crate) fn pixel_summary(p: &PixelInfo) -> String {
    let value = p.display.as_ref().map_or_else(|| "…".to_owned(), srgb_text);
    t!(
        "inspector-a11y",
        x = p.x.to_string(),
        y = p.y.to_string(),
        value = value
    )
}

/// Draws the open panels and records where the loupe goes.
pub(crate) fn draw(ch: &mut Chrome, cx: &mut Ctx, settings: &Settings) {
    ch.inspect.loupe = None;
    let tools = ch.inspect.tools;
    let bar = colour_bar_shown(&ch.inspect, settings);
    if !tools.histogram && !tools.pixel && !bar {
        return;
    }
    let line = 17.0 * cx.style(1.0).size;
    let w = GRAPH_W + 2.0 * PAD;
    let mut bottom = stack_bottom(cx.h, ch.inspect.floor);
    if let (true, Some(l)) = (bar, ch.inspect.legend.as_ref()) {
        let r = draw_colour_bar(cx, l, w, line, bottom);
        bottom = r.y - metrics::GAP;
    }
    if tools.pixel {
        let r = draw_pixel(ch, cx, w, line, bottom);
        bottom = r.y - metrics::GAP;
    }
    if tools.histogram {
        draw_histogram(ch, cx, settings, w, line, bottom);
    }
}

/// Where the lowest inspection panel ends: clear of the toolbar, and above the application's
/// own panels in the corner (`floor`).
pub(crate) fn stack_bottom(h: f32, floor: Option<f32>) -> f32 {
    let clear = h - BOTTOM;
    floor.map_or(clear, |y| clear.min(y - metrics::GAP))
}

fn panel(cx: &mut Ctx, r: Rect) {
    cx.dl
        .rect(r, metrics::RADIUS_MD, cx.pal.tooltip_bg.with_alpha(0.82));
}

fn draw_histogram(
    ch: &Chrome,
    cx: &mut Ctx,
    settings: &Settings,
    w: f32,
    line: f32,
    bottom: f32,
) -> Rect {
    let ts = cx.style(metrics::FONT_SMALL);
    let h = PAD + line + 4.0 + GRAPH_H + 4.0 + line + PAD * 0.5;
    let r = cx.m(Rect::new(14.0, bottom - h, w, h));
    panel(cx, r);
    let text = cx.pal.tooltip_text;
    let title = cx.m_in(
        Rect::new(r.x + PAD, r.y + PAD * 0.5, w - 2.0 * PAD, line),
        r,
    );
    cx.label(
        &t!("histogram-title"),
        cx.style(metrics::FONT_LABEL).weight(600.0),
        title,
        HAlign::Start,
        text,
    );
    let g = Rect::new(r.x + PAD, title.bottom() + 4.0, GRAPH_W, GRAPH_H);
    cx.dl.rect(g, 3.0, Color::BLACK.with_alpha(0.28));
    // Quarter-tone guides.
    for q in 1..4 {
        let x = g.x + g.w * q as f32 / 4.0;
        cx.dl.line((x, g.y), (x, g.bottom()), 1.0, text.fade(0.12));
    }
    let foot = Rect::new(r.x + PAD, g.bottom() + 4.0, w - 2.0 * PAD, line);
    match ch.inspect.histogram.as_deref().filter(|h| !h.is_empty()) {
        Some(hist) => {
            let log = settings.inspection.histogram_log;
            let chans = channels(settings.inspection.histogram_channels);
            let peak = hist.peak(chans, log);
            let cw = g.w / COLUMNS as f32;
            for &c in chans {
                let color = curve_color(c, cx.pal);
                for (i, v) in hist.curve(c, log, peak, COLUMNS).into_iter().enumerate() {
                    if v <= 0.0 {
                        continue;
                    }
                    let bh = (v * g.h).max(1.0);
                    let bar = Rect::new(g.x + i as f32 * cw, g.bottom() - bh, cw, bh);
                    cx.dl.rect(bar, 0.0, color);
                }
            }
            let s = clipped_text(hist);
            let foot = cx.m_in(foot, r);
            cx.label_fit(&s, ts, foot, HAlign::Start, text.fade(0.75), false);
        }
        None => {
            let foot = cx.m_in(foot, r);
            cx.label(
                &t!("histogram-empty"),
                ts,
                foot,
                HAlign::Start,
                text.fade(0.6),
            );
        }
    }
    r
}

fn draw_pixel(ch: &mut Chrome, cx: &mut Ctx, w: f32, line: f32, bottom: f32) -> Rect {
    let ls = cx.style(metrics::FONT_SMALL);
    let vs = cx.style(metrics::FONT_MONO).tabular().weight(550.0);
    let text = cx.pal.tooltip_text;
    let pixel = ch.inspect.pixel;
    let rows = pixel.as_ref().map(pixel_rows).unwrap_or_default();
    let note = match &pixel {
        None => Some(t!("inspector-outside")),
        Some(p) if p.display.is_some_and(|d| !d.exact) => Some(t!("inspector-approximate")),
        _ => None,
    };
    let inner = w - 2.0 * PAD;
    let note_block = note.as_ref().map(|s| cx.layout(s, ls, Some(inner)));
    let note_h = note_block.as_ref().map_or(0.0, |b| b.height + 6.0);
    let h = PAD * 0.5 + line + 6.0 + LOUPE + 8.0 + rows.len() as f32 * line + note_h + PAD * 0.5;
    let r = cx.m(Rect::new(14.0, bottom - h, w, h));
    // The panel around the loupe hole: above, below and beside it.
    let title = cx.m_in(Rect::new(r.x + PAD, r.y + PAD * 0.5, inner, line), r);
    let loupe = cx.m_in(Rect::new(r.x + PAD, title.bottom() + 6.0, LOUPE, LOUPE), r);
    let bg = cx.pal.tooltip_bg.with_alpha(0.82);
    let rad = metrics::RADIUS_MD;
    cx.dl.rect_full(
        Rect::from_min_max(r.x, r.y, r.right(), loupe.y),
        [rad, rad, 0.0, 0.0],
        bg,
        Color::TRANSPARENT,
        0.0,
    );
    cx.dl.rect_full(
        Rect::from_min_max(r.x, loupe.bottom(), r.right(), r.bottom()),
        [0.0, 0.0, rad, rad],
        bg,
        Color::TRANSPARENT,
        0.0,
    );
    let (sx0, sx1) = (r.x, loupe.x);
    let (ex0, ex1) = (loupe.right(), r.right());
    for (a, b) in [(sx0, sx1), (ex0, ex1)] {
        cx.dl
            .rect(Rect::from_min_max(a, loupe.y, b, loupe.bottom()), 0.0, bg);
    }
    cx.label(
        &t!("inspector-title"),
        cx.style(metrics::FONT_LABEL).weight(600.0),
        title,
        HAlign::Start,
        text,
    );
    if pixel.is_some() {
        ch.inspect.loupe = Some(loupe);
        // The inspected pixel is the centre one.
        let c = loupe.center();
        let cell = 9.0;
        cx.dl.stroke_rect(
            Rect::new(c.0 - cell * 0.5, c.1 - cell * 0.5, cell, cell),
            0.0,
            Color::WHITE,
            1.5,
        );
        cx.dl.stroke_rect(
            Rect::new(
                c.0 - cell * 0.5 - 1.5,
                c.1 - cell * 0.5 - 1.5,
                cell + 3.0,
                cell + 3.0,
            ),
            0.0,
            Color::BLACK.with_alpha(0.7),
            1.0,
        );
    } else {
        cx.dl.rect(loupe, 0.0, Color::BLACK.with_alpha(0.35));
    }
    cx.dl.stroke_rect(loupe, 0.0, text.fade(0.35), 1.0);
    // Beside the loupe (end side): the colour swatch and its hex code.
    if let Some(d) = pixel.as_ref().and_then(|p| p.display) {
        let sx = r.x + PAD + LOUPE + PAD;
        let side = Rect::new(sx, loupe.y, r.right() - PAD - sx, LOUPE);
        let sw = cx.m_in(Rect::new(side.x, side.y, 56.0, 56.0), r);
        let side = cx.m_in(side, r);
        let col = Color::rgba(
            d.srgb[0] as f32 / 255.0,
            d.srgb[1] as f32 / 255.0,
            d.srgb[2] as f32 / 255.0,
            1.0,
        );
        cx.dl
            .rect_border(sw, metrics::RADIUS_SM, col, text.fade(0.4), 1.0);
        let hx = Rect::new(side.x, sw.bottom() + 6.0, side.w, line);
        cx.label(
            &hex([d.srgb[0], d.srgb[1], d.srgb[2]]),
            vs,
            hx,
            HAlign::Start,
            cx.pal.hud_text,
        );
    }
    let mut y = loupe.bottom() + 8.0;
    for (l, v) in &rows {
        let row = Rect::new(r.x + PAD, y, inner, line);
        cx.label(l, ls, row, HAlign::Start, text.fade(0.7));
        cx.label_fit(v, vs, row, HAlign::End, cx.pal.hud_text, false);
        y += line;
    }
    if let Some(b) = note_block {
        let nr = Rect::new(r.x + PAD, y + 3.0, inner, b.height);
        cx.block_top(&b, nr, HAlign::Start, text.fade(0.6));
    }
    r
}

/// Gradient height (logical px).
const BAR_H: f32 = 14.0;

/// The colour bar panel: title (map and unit), the gradient from the legend's low end to its
/// high end, and tick values below it. Returns its rectangle.
fn draw_colour_bar(cx: &mut Ctx, l: &Legend, w: f32, line: f32, bottom: f32) -> Rect {
    let ts = cx.style(metrics::FONT_SMALL).tabular();
    let text = cx.pal.tooltip_text;
    let inner = w - 2.0 * PAD;
    let h = PAD * 0.5 + line + 4.0 + BAR_H + 4.0 + line + PAD * 0.5;
    let r = cx.m(Rect::new(14.0, bottom - h, w, h));
    panel(cx, r);
    let title = cx.m_in(Rect::new(r.x + PAD, r.y + PAD * 0.5, inner, line), r);
    cx.label_fit(
        &colour_bar_title(l),
        cx.style(metrics::FONT_LABEL).weight(600.0),
        title,
        HAlign::Start,
        text,
        false,
    );
    // The gradient: one column per two table entries (a few dozen rectangles a frame).
    let g = Rect::new(r.x + PAD, title.bottom() + 4.0, inner, BAR_H);
    let cols = specular_inspect::MAP_LEN / 2;
    let cw = g.w / cols as f32;
    for i in 0..cols {
        let c = l.map.sample((i as f32 + 0.5) / cols as f32);
        // Overlap by a hair so no seams show between columns.
        let col = Rect::new(g.x + i as f32 * cw, g.y, cw + 0.5, g.h);
        cx.dl.rect(col, 0.0, Color::rgba(c[0], c[1], c[2], 1.0));
    }
    cx.dl.stroke_rect(g, 0.0, text.fade(0.35), 1.0);
    // Ticks: as many as fit at about 48 px apart.
    let max = ((g.w / 48.0) as usize).clamp(2, 8);
    let ty = g.bottom() + 4.0;
    for (v, s) in colour_bar_ticks(l, max) {
        let x = g.x + l.position(v) as f32 * g.w;
        cx.dl
            .line((x, g.bottom() - 4.0), (x, g.bottom()), 1.0, text.fade(0.8));
        let b = cx.layout(&s, ts, None);
        let lx = (x - b.width * 0.5).clamp(r.x + 4.0, r.right() - 4.0 - b.width);
        cx.label(
            &s,
            ts,
            Rect::new(lx, ty, b.width, line),
            HAlign::Center,
            text.fade(0.85),
        );
    }
    r
}
