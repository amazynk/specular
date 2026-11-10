//! Cull mode (DESIGN.md §6.3): `Ctrl+Shift+K` from the viewer or the gallery, or on its own
//! when a folder that is mostly RAW files is opened (`cull.auto_enter`). Full screen with the
//! chrome hidden and a compact strip at the bottom: the file, its position, the exposure line
//! (shutter, aperture, ISO, focal length) and the marks, which also take clicks.
//!
//! `p`, `x` and `u` pick, reject and unflag; `1`–`5` rate (`0` clears) and `6`–`9` label (a
//! second press takes the label off): the cull layer of the keymap, rebindable. Each mark is
//! confirmed on screen and to the screen reader, goes through the gallery's marks (its tables and, with `gallery.rating_sidecars`,
//! the XMP sidecars), can be undone with Ctrl+Z, and a pick, a reject or a rating moves on to
//! the next image (`cull.auto_advance`).
//!
//! A RAW and a JPEG sharing a stem are one item ([`group`]): the viewer's listing holds one
//! file per item (`cull.show` picks which), so stepping and prefetch work as usual, and a mark
//! applies to every member. Leaving puts the full listing back at the image culled last.
//!
//! Alt+1–3 copy the current item (its members and their XMP sidecars) to the folders set in
//! `cull.slot_1`–`slot_3`, Alt+Shift+1–3 move it there ([`slots`]); both can be undone.
//! Consecutive shots of one burst collapse into a stack showing one frame ([`burst`]): `s`
//! opens or closes it, `b` picks the frame on screen and rejects the rest. Files appearing in
//! the folder join the run once written ([`hot`]); `t` follows them as they arrive.

pub(super) mod astro;
#[cfg(test)]
mod astro_shots;
mod burst;
mod group;
mod hot;
#[cfg(test)]
mod shots;
mod slots;
mod subframes;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use crossbeam_channel::{Receiver, Sender};
use specular_gfx::{Align, DrawList, Icon, Rect, Text, TextStyle, draw_icon};
use specular_i18n::t;
use specular_input::{Action, Mode};
use specular_log::{UI, debug};
use specular_settings::{CullCollision, CullShow};
use specular_theme::metrics::{FONT_BODY, FONT_TITLE, PAD, RADIUS_MD, RADIUS_SM, TOOLBAR_MARGIN};
use specular_theme::{Color, Palette, durations};
use specular_thumbs::watch::TreeWatch;
use specular_widgets::accesskit;

use self::burst::{Capture, Stack};
use self::group::{Item, Show};
use self::slots::{Collision, Outcome, Placed};
use super::gallery::{Mark, mark_label_name, marks};
use super::{ActiveEventLoop, App};

/// How long a mark's confirmation stays (s), the last part of it fading out.
const FLASH_S: f64 = 0.9;
const FLASH_FADE_S: f64 = 0.25;
/// AccessKit ids of the strip (this one) and its buttons (after it).
const A11Y_BASE: u64 = 0x5000_0000_0000_0000;
/// The stars' colour, as in the gallery.
const STAR: Color = Color::hex(0xFFD60A);
/// The labels' colours (1–4), as in the gallery.
const LABEL_COLOURS: [u32; 4] = [0xE5484D, 0xF5D90A, 0x46A758, 0x3E63DD];

/// A control on the strip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Hit {
    Star(u8),
    Pick,
    Reject,
    Exit,
    /// Opens or closes the burst on screen.
    Stack,
    /// The colour label: shown, not a button (it has a tooltip, no click).
    Label,
}

impl Hit {
    /// Whether a click on it does something (a hand pointer, a hover look, a button for
    /// screen readers).
    pub fn is_button(self) -> bool {
        self != Hit::Label
    }
}

/// Items' captures read, by their first member.
type Captures = Vec<(PathBuf, Option<Capture>)>;

struct Run {
    /// The listing as it was (every file), put back on leaving; moved files leave it and new
    /// ones join it.
    listing: Vec<PathBuf>,
    items: Vec<Item>,
    show: Show,
    entered_fullscreen: bool,
    from_gallery: bool,
    /// The last mark's confirmation and when it was made.
    flash: Option<(String, f64)>,
    sidecars: Option<Receiver<Vec<(PathBuf, Mark)>>>,
    /// The folder culled: new files in it join the run.
    dir: Option<PathBuf>,
    /// Each item's capture, by its first member (`None`: it has no capture time).
    caps: HashMap<PathBuf, Option<Capture>>,
    /// Captures being read.
    caps_rx: Vec<Receiver<Captures>>,
    /// The bursts over `items`, and what they were made with (`cull.bursts`, the gap).
    stacks: Vec<Stack>,
    stacked_with: Option<(bool, f64)>,
    /// Opened bursts, and the frame each collapsed one shows, by their first item's key.
    open: HashSet<PathBuf>,
    tops: HashMap<PathBuf, PathBuf>,
    watch: Option<TreeWatch>,
    settle: hot::Settle,
    /// Jump to new files as they arrive.
    follow: bool,
    /// Astronomy subframes in the run: their facts, the table and the chart.
    astro: Option<astro::State>,
}

/// A copy or move to a slot made, for undo.
struct SlotUndo {
    copy: bool,
    placed: Vec<Placed>,
    /// Where the item was in the listing.
    at: usize,
}

/// A file operation finished on its thread.
enum Op {
    Placed {
        copy: bool,
        folder: PathBuf,
        outcome: Outcome,
        /// The item's members (held while it ran).
        members: Vec<PathBuf>,
    },
    /// The rejected frames moved to a subfolder.
    Rejects {
        folder: PathBuf,
        outcomes: Vec<(Vec<PathBuf>, Outcome)>,
    },
    Undone {
        copy: bool,
        at: usize,
        placed: Vec<Placed>,
        /// Moves: the files back in place. Copies: the copies removed.
        done: Vec<PathBuf>,
        kept: Vec<PathBuf>,
        failed: Vec<(PathBuf, String)>,
    },
}

/// `SPECULAR_BENCH_CULL=steps[,interval_ms]`: enters cull mode on the first folder listed,
/// steps through `steps` items (no marks) and prints how long each took to show.
struct Bench {
    steps: usize,
    interval: f64,
    started: bool,
    step_at: Option<Instant>,
    sharp_ms: Option<f64>,
    first: Option<(f64, f64)>,
    open: Vec<f64>,
    sharp: Vec<f64>,
}

#[derive(Default)]
pub(super) struct Cull {
    run: Option<Run>,
    /// A folder opened explicitly: cull mode starts once it is listed, if it is mostly RAW.
    pub auto_dir: Option<PathBuf>,
    /// The strip and its controls as last drawn (logical px).
    bar: Option<Rect>,
    hits: Vec<(Rect, Hit)>,
    /// The control under the pointer and since when (s), and whether a click or key press
    /// put its tooltip away (until the pointer moves to another control).
    hover: Option<(Hit, f64)>,
    tip_off: bool,
    bench: Option<Bench>,
    /// File operations: their results, how many are under way, the files they hold, and
    /// the undo of each done.
    ops: Option<(Sender<Op>, Receiver<Op>)>,
    ops_running: usize,
    busy: HashSet<PathBuf>,
    undo: HashMap<u64, SlotUndo>,
    /// Folders that cannot be watched (not tried again).
    watch_failed: HashSet<PathBuf>,
    /// The subframe panel and its controls as last drawn (logical px).
    panel: Option<Rect>,
    panel_hits: Vec<(Rect, astro::PanelHit)>,
}

impl Cull {
    /// Whether cull mode is on (its keymap layer then comes first).
    pub(super) fn active(&self) -> bool {
        self.run.is_some()
    }

    pub fn from_env() -> Cull {
        let bench = std::env::var("SPECULAR_BENCH_CULL").ok().and_then(|v| {
            let mut it = v.split(',');
            let steps = it.next()?.trim().parse().ok()?;
            let interval = it
                .next()
                .and_then(|s| s.trim().parse::<f64>().ok())
                .unwrap_or(600.0);
            Some(Bench {
                steps,
                interval: interval / 1000.0,
                started: false,
                step_at: None,
                sharp_ms: None,
                first: None,
                open: Vec::new(),
                sharp: Vec::new(),
            })
        });
        Cull {
            bench,
            ..Cull::default()
        }
    }

    /// Where the subframe panel and its controls were drawn (logical px), for clicks.
    pub fn set_panel(&mut self, drawn: Option<(Rect, Vec<(Rect, astro::PanelHit)>)>) {
        match drawn {
            Some((r, hits)) => {
                self.panel = Some(r);
                self.panel_hits = hits;
            }
            None => {
                self.panel = None;
                self.panel_hits.clear();
            }
        }
    }

    /// Where the strip and its controls were drawn (logical px), for clicks.
    pub fn set_hits(&mut self, bar: Rect, hits: Vec<(Rect, Hit)>) {
        self.bar = Some(bar);
        self.hits = hits;
    }

    /// Follows the pointer (logical px; `None`: it left the window) over the strip's
    /// controls as last drawn. True when the control under it changed.
    pub fn hover_at(&mut self, p: Option<(f32, f32)>, now: f64) -> bool {
        let h = p.filter(|_| self.run.is_some()).and_then(|p| {
            self.hits
                .iter()
                .find(|(r, _)| r.contains(p))
                .map(|(_, h)| *h)
        });
        if h == self.hover.map(|h| h.0) {
            return false;
        }
        self.hover = h.map(|h| (h, now));
        self.tip_off = false;
        true
    }

    /// The control under the pointer.
    pub fn hovered(&self) -> Option<Hit> {
        self.hover.map(|h| h.0)
    }

    /// Whether the pointer is over a control that acts on a click.
    pub fn over_button(&self) -> bool {
        self.hovered().is_some_and(Hit::is_button)
    }

    /// The tooltip's control once the pointer has rested on it for the tooltip delay, or
    /// how long until then (s; infinite: no tooltip coming).
    fn tip_due(&self, now: f64) -> Result<Hit, f64> {
        let Some((h, since)) = self.hover.filter(|_| !self.tip_off) else {
            return Err(f64::INFINITY);
        };
        let wait = f64::from(durations::TOOLTIP_DELAY) - (now - since);
        if wait <= 0.0 { Ok(h) } else { Err(wait) }
    }
}

/// A control's tooltip: its name and its key ("3 stars (3)", "Pick (p)"). The marks' keys come
/// from the keymap's cull layer, the labels' as a run ("6–9") as the cheat sheet lists them;
/// leaving cull mode is a viewer action in the active modes. A remapped key shows; an unbound
/// action shows no key.
pub(super) fn tip_text(h: Hit, s: &Strip, keymap: &specular_input::Keymap) -> String {
    let cull = |a: Action| keymap.keys_in(Mode::Cull, a).first().map(|k| k.to_string());
    let (label, key) = match h {
        Hit::Star(n) => (
            t!("gallery-stars", stars = i64::from(n)),
            cull(Action::CullRate(n)),
        ),
        Hit::Pick => (t!("gallery-pick"), cull(Action::CullPick)),
        Hit::Reject => (t!("gallery-reject"), cull(Action::CullReject)),
        Hit::Stack => (
            if s.burst.is_some_and(|b| b.2) {
                t!("cull-tip-burst-close")
            } else {
                t!("cull-tip-burst-open")
            },
            cull(Action::CullBurst),
        ),
        Hit::Exit => (
            t!("cull-tip-exit"),
            keymap
                .primary_key(Action::ToggleCull)
                .map(|k| k.to_string()),
        ),
        Hit::Label => (
            match s.mark.label {
                0 => mark_label_name(0),
                l => t!("cull-tip-label", label = mark_label_name(l)),
            },
            {
                let keys: Vec<String> =
                    (1..=4).filter_map(|l| cull(Action::CullLabel(l))).collect();
                specular_widgets::digit_run(&keys).map(|k| k.join(", "))
            },
        ),
    };
    match key {
        Some(k) => t!("tooltip-with-shortcut", label = label, shortcut = k),
        None => label,
    }
}

/// What the strip shows.
pub(super) struct Strip {
    /// The item: "IMG_0001 · JPG + CR3", or the file name.
    pub title: String,
    /// Position in the items, 1-based, and their count.
    pub position: (usize, usize),
    pub exposure: Option<String>,
    /// Local AI's culling flags: "Eyes closed", "Blurred", "Dark", … (none without it).
    pub flags: Vec<String>,
    pub mark: Mark,
    /// The burst on screen: the frame's place in it (1-based), its frames, and whether it is
    /// open.
    pub burst: Option<(usize, usize, bool)>,
    /// The last mark's confirmation and its opacity.
    pub flash: Option<(String, f32)>,
    /// The control under the pointer (a hover look; over a star, the rating it would set),
    /// and the tooltip shown for it once the pointer has rested there.
    pub hover: Option<Hit>,
    pub tip: Option<String>,
    /// The theme's palette (the flags' fills, the tooltip).
    pub pal: Palette,
}

impl Strip {
    /// "Burst 2 of 5", or "Burst of 5" while it is closed.
    fn burst_text(&self) -> Option<String> {
        let (i, n, open) = self.burst?;
        Some(if open {
            t!("cull-burst-open", index = i as i64, count = n as i64)
        } else {
            t!("cull-burst-closed", count = n as i64)
        })
    }

    /// The second line: "3 / 120 · Burst 2 of 5 · 1/250 s · f/8 · ISO 100 · 35 mm".
    fn detail(&self) -> String {
        let mut parts = vec![format!("{} / {}", self.position.0, self.position.1)];
        parts.extend(self.burst_text());
        parts.extend(self.exposure.clone());
        specular_i18n::dot_join(&parts)
    }

    /// The strip for a screen reader: everything it shows, in words.
    fn label(&self) -> String {
        let mut parts = vec![self.title.clone()];
        parts.push(t!(
            "readout-position-a11y",
            index = self.position.0 as i64,
            count = self.position.1 as i64
        ));
        parts.extend(self.burst_text());
        parts.extend(self.exposure.clone());
        parts.extend(self.flags.iter().cloned());
        parts.extend(mark_words(self.mark));
        parts.join(", ")
    }
}

/// The marks in words, unset ones left out.
fn mark_words(m: Mark) -> Vec<String> {
    let mut v = Vec::new();
    match m.flag {
        1 => v.push(t!("gallery-pick")),
        -1 => v.push(t!("gallery-reject")),
        _ => {}
    }
    if m.rating > 0 {
        v.push(t!("gallery-stars", stars = i64::from(m.rating)));
    }
    if m.label > 0 {
        v.push(mark_label_name(m.label));
    }
    v
}

/// The strip across the bottom of the image `area` (logical x, y, w, h): the item and its
/// details at the start, the stars, flags, label and the way out at the end, and the last
/// mark's confirmation above it. Returns the strip and its controls.
pub(super) fn draw_strip(
    dl: &mut DrawList,
    text: &mut Text,
    s: &Strip,
    area: (f32, f32, f32, f32),
    rtl: bool,
    ts: f32,
) -> (Rect, Vec<(Rect, Hit)>) {
    let (ax, ay, aw, ah) = area;
    let m = TOOLBAR_MARGIN;
    let title_st = TextStyle::new(FONT_TITLE * ts).weight(600.0);
    let body_st = TextStyle::new(FONT_BODY * ts);
    let b = 30.0 * ts;
    let gap = 4.0 * ts;
    let sep = 14.0 * ts;
    // The controls, from the strip's start towards its end: the burst's (in one), the stars,
    // the flags, the colour label (shown, not a button) and the way out.
    #[derive(Clone, Copy, PartialEq)]
    enum Slot {
        Gap,
        Button(Hit),
    }
    let burst: &[(f32, Slot)] = if s.burst.is_some() {
        &[(b, Slot::Button(Hit::Stack)), (sep, Slot::Gap)]
    } else {
        &[]
    };
    let controls: Vec<(f32, Slot)> = burst
        .iter()
        .copied()
        .chain((1..=5).map(|n| (b, Slot::Button(Hit::Star(n)))))
        .chain([
            (sep, Slot::Gap),
            (b, Slot::Button(Hit::Pick)),
            (b, Slot::Button(Hit::Reject)),
            (b, Slot::Button(Hit::Label)),
            (sep, Slot::Gap),
            (b, Slot::Button(Hit::Exit)),
        ])
        .collect();
    let cw: f32 = controls.iter().map(|c| c.0).sum();
    let max_w = (aw - 2.0 * m - 2.0 * PAD - cw - sep).max(40.0);
    let title = text.layout(&s.title, title_st, Some(max_w), Align::Start);
    let detail = text.layout(&s.detail(), body_st, Some(max_w), Align::Start);
    let flags = (!s.flags.is_empty()).then(|| {
        text.layout(
            &specular_i18n::dot_join(&s.flags),
            body_st,
            Some(max_w),
            Align::Start,
        )
    });
    let flags_h = flags.as_ref().map_or(0.0, |f| gap + f.height);
    let h = (title.height + gap + detail.height + flags_h).max(b) + 2.0 * PAD;
    let bar = Rect::new(ax, ay + ah - h, aw, h);
    dl.rect(bar, 0.0, Color::rgba(0.0, 0.0, 0.0, 0.62));

    // The item and its details.
    let tx = |w: f32| if rtl { ax + aw - m - w } else { ax + m };
    let ty = bar.y + PAD;
    text.draw(dl, &title, tx(title.width), ty, Color::WHITE);
    text.draw(
        dl,
        &detail,
        tx(detail.width),
        ty + title.height + gap,
        Color::WHITE.with_alpha(0.8),
    );
    if let Some(f) = &flags {
        let y = ty + title.height + gap + detail.height + gap;
        // The bar is dark in both themes, so the dark warning colour reads on it.
        text.draw(dl, f, tx(f.width), y, Palette::dark().warning);
    }

    // The controls, at the end.
    let cy = bar.y + (h - b) * 0.5;
    let mut hits = Vec::new();
    let mut off = 0.0;
    for (w, slot) in &controls {
        let hit = match slot {
            Slot::Button(h) => Some(*h),
            _ => None,
        };
        let hit = &hit;
        let x = if rtl {
            ax + m + cw - off - w
        } else {
            ax + aw - m - cw + off
        };
        off += w;
        let r = Rect::new(x, cy, *w, b);
        let icon = r.inset(6.0 * ts);
        // The toolbar's hover look (the strip is dark whatever the theme).
        if hit.is_some_and(|h| h.is_button() && s.hover == Some(h)) {
            dl.rect(r, RADIUS_SM + 2.0, Palette::dark().surface_hover);
        }
        match hit {
            Some(Hit::Star(n)) => {
                // Over a star, the rating a click sets: the stars up to it lit; the ones
                // past it that are set now stay faintly lit.
                let c = match s.hover {
                    Some(Hit::Star(k)) if *n <= k => STAR,
                    Some(Hit::Star(_)) if *n <= s.mark.rating => STAR.with_alpha(0.4),
                    _ if *n <= s.mark.rating => STAR,
                    _ => Color::WHITE.with_alpha(0.35),
                };
                let st = TextStyle::new(FONT_TITLE * 1.2 * ts);
                let g = text.layout("★", st, None, Align::Start);
                text.draw(
                    dl,
                    &g,
                    r.x + (r.w - g.width) * 0.5,
                    r.y + (r.h - g.height) * 0.5,
                    c,
                );
            }
            Some(Hit::Pick) | Some(Hit::Reject) => {
                let pick = *hit == Some(Hit::Pick);
                let set = s.mark.flag == if pick { 1 } else { -1 };
                let (glyph, fill) = if pick {
                    (Icon::Check, s.pal.accent)
                } else {
                    (Icon::Close, s.pal.danger)
                };
                if set {
                    dl.circle((r.x + r.w * 0.5, r.y + r.h * 0.5), r.w * 0.45, fill);
                }
                let c = if set {
                    Color::WHITE
                } else {
                    Color::WHITE.with_alpha(0.6)
                };
                draw_icon(dl, glyph, icon, c, 2.0 * ts, false);
            }
            Some(Hit::Exit) => {
                draw_icon(
                    dl,
                    Icon::ExitFullscreen,
                    icon,
                    Color::WHITE.with_alpha(0.85),
                    2.0 * ts,
                    rtl,
                );
            }
            Some(Hit::Stack) => {
                let open = s.burst.is_some_and(|b| b.2);
                if open {
                    dl.circle(
                        (r.x + r.w * 0.5, r.y + r.h * 0.5),
                        r.w * 0.45,
                        Color::WHITE.with_alpha(0.2),
                    );
                }
                draw_icon(
                    dl,
                    Icon::Copy,
                    icon,
                    Color::WHITE.with_alpha(0.85),
                    2.0 * ts,
                    rtl,
                );
            }
            Some(Hit::Label) => {
                // The colour label.
                let c = (r.x + r.w * 0.5, r.y + r.h * 0.5);
                match s.mark.label {
                    l @ 1..=4 => dl.circle(c, 7.0 * ts, Color::hex(LABEL_COLOURS[l as usize - 1])),
                    _ => dl.stroke_rect(
                        Rect::new(c.0 - 6.0 * ts, c.1 - 6.0 * ts, 12.0 * ts, 12.0 * ts),
                        6.0 * ts,
                        Color::WHITE.with_alpha(0.35),
                        1.5 * ts,
                    ),
                }
            }
            None => {}
        }
        if let Some(hit) = hit {
            hits.push((r, *hit));
        }
    }

    // The last mark, confirmed above the strip.
    if let Some((msg, a)) = &s.flash {
        let block = text.layout(msg, title_st, Some(aw * 0.8), Align::Start);
        let (pw, ph) = (block.width + 4.0 * PAD, block.height + 2.0 * PAD);
        let r = Rect::new(ax + (aw - pw) * 0.5, bar.y - m - ph, pw, ph);
        dl.rect(
            r,
            RADIUS_MD.max(ph * 0.5),
            Color::rgba(0.0, 0.0, 0.0, 0.7 * a),
        );
        text.draw(
            dl,
            &block,
            r.x + 2.0 * PAD,
            r.y + PAD,
            Color::WHITE.with_alpha(*a),
        );
    }

    // The tooltip of the control under the pointer, above the strip (the strip sits at the
    // bottom: the shared tooltip flips up there), inside the area.
    if let (Some(tip), Some((r, _))) = (
        &s.tip,
        s.hover.and_then(|h| hits.iter().find(|(_, q)| *q == h)),
    ) {
        let bounds = Rect::new(ax, ay, aw, ah);
        specular_widgets::draw_tooltip(dl, text, &s.pal, tip, tip_anchor(bar, *r), bounds, ts);
    }
    (bar, hits)
}

/// What a control's tooltip is placed against: the control's column across the whole strip,
/// so the tooltip clears the strip rather than covering its top.
pub(super) fn tip_anchor(bar: Rect, control: Rect) -> Rect {
    Rect::new(control.x, bar.y, control.w, bar.h)
}

/// The strip's AccessKit nodes: a polite status with everything it shows, and its buttons.
pub(super) fn a11y(
    tree: &mut accesskit::TreeUpdate,
    s: &Strip,
    bar: Option<Rect>,
    hits: &[(Rect, Hit)],
    scale: f64,
) {
    use accesskit::{Node, NodeId, Role};
    let bounds = |r: Rect| accesskit::Rect {
        x0: f64::from(r.x) * scale,
        y0: f64::from(r.y) * scale,
        x1: f64::from(r.x + r.w) * scale,
        y1: f64::from(r.y + r.h) * scale,
    };
    let mut strip = Node::new(Role::Status);
    strip.set_live(accesskit::Live::Polite);
    strip.set_label(s.label());
    if let Some(r) = bar {
        strip.set_bounds(bounds(r));
    }
    let mut ids = Vec::new();
    let mut nodes = Vec::new();
    // Ids follow the place in `hits` (clicks from a screen reader find the control by it).
    for (i, (r, hit)) in hits.iter().enumerate().filter(|(_, h)| h.1.is_button()) {
        let id = NodeId(A11Y_BASE + 1 + i as u64);
        let mut n = Node::new(Role::Button);
        n.set_label(match hit {
            Hit::Star(k) => t!("gallery-stars", stars = i64::from(*k)),
            Hit::Pick => t!("gallery-pick"),
            Hit::Reject => t!("gallery-reject"),
            Hit::Exit => t!("palette-cull"),
            Hit::Stack => t!("cull-burst-toggle"),
            Hit::Label => continue,
        });
        let on = match hit {
            Hit::Pick => Some(s.mark.flag == 1),
            Hit::Reject => Some(s.mark.flag == -1),
            Hit::Star(k) => Some(*k <= s.mark.rating),
            Hit::Stack => Some(s.burst.is_some_and(|b| b.2)),
            Hit::Exit | Hit::Label => None,
        };
        if let Some(on) = on {
            n.set_toggled(if on {
                accesskit::Toggled::True
            } else {
                accesskit::Toggled::False
            });
        }
        n.set_bounds(bounds(*r));
        n.add_action(accesskit::Action::Click);
        ids.push(id);
        nodes.push((id, n));
    }
    strip.set_children(ids);
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(A11Y_BASE));
    }
    tree.nodes.push((NodeId(A11Y_BASE), strip));
    tree.nodes.extend(nodes);
}

fn pct(v: &mut [f64], q: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v.get(((v.len() as f64 - 1.0) * q).round() as usize)
        .copied()
        .unwrap_or(0.0)
}

fn same_dir(a: &Path, b: &Path) -> bool {
    a == b
        || std::fs::canonicalize(a)
            .ok()
            .is_some_and(|a| std::fs::canonicalize(b).ok() == Some(a))
}

impl App {
    fn cull_show(&self) -> Show {
        match self.settings.cull.show {
            CullShow::Raw => Show::Raw,
            _ => Show::Jpeg,
        }
    }

    fn cull_current_path(&self) -> Option<PathBuf> {
        self.current.and_then(|i| self.entries.get(i)).cloned()
    }

    /// Whether a folder listed now enters cull mode: the benchmark's first, or the folder
    /// opened with `cull.auto_enter` on when most of its frames have a RAW.
    fn cull_wanted(&mut self, dir: Option<&Path>, files: &[PathBuf]) -> bool {
        if self.cull.run.is_some() {
            return false;
        }
        if let Some(b) = self.cull.bench.as_mut()
            && !b.started
        {
            b.started = true;
            return true;
        }
        let Some(auto) = self.cull.auto_dir.clone() else {
            return false;
        };
        if dir.is_some_and(|d| !same_dir(d, &auto)) {
            return false;
        }
        self.cull.auto_dir = None;
        group::mostly_raw(&group::group(files, self.cull_show(), None))
    }

    /// The gallery has listed its folder (cull mode may start).
    pub(super) fn cull_gallery_listed(&mut self) {
        if self.cull.run.is_some() {
            return;
        }
        let dir = self.gallery_dir().map(Path::to_path_buf);
        let images = self.gallery_images();
        if self.cull_wanted(dir.as_deref(), &images) && !images.is_empty() {
            self.cull_enter();
        }
    }

    /// `Event::Folder`: the viewer's listing. In cull mode (or entering it now) the files
    /// become items, one shown file each.
    pub(super) fn cull_listed(&mut self, dir: &Path, files: Vec<PathBuf>) -> Vec<PathBuf> {
        let entering = !files.is_empty() && self.cull_wanted(Some(dir), &files);
        if entering && !self.in_book() {
            self.slideshow_stop();
            self.gallery_marks_ready();
            self.cull_begin(Vec::new(), Vec::new(), false, Some(dir.to_path_buf()));
        }
        let keep = self.cull_current_path();
        let Some(run) = self.cull.run.as_mut() else {
            return files;
        };
        run.listing = files;
        if run.dir.is_none() {
            run.dir = Some(dir.to_path_buf());
        }
        let shown = self.cull_rebuild(keep.as_deref());
        self.cull_read_sidecars();
        self.cull_read_captures();
        self.cull_watch();
        shown
    }

    /// Full screen, the chrome away, the run started (the listing follows).
    fn cull_begin(
        &mut self,
        listing: Vec<PathBuf>,
        items: Vec<Item>,
        from_gallery: bool,
        dir: Option<PathBuf>,
    ) {
        let entered_fullscreen = !self.fullscreen;
        if entered_fullscreen {
            self.toggle_fullscreen();
        }
        self.cull.run = Some(Run {
            listing,
            items,
            show: self.cull_show(),
            entered_fullscreen,
            from_gallery,
            flash: None,
            sidecars: None,
            dir,
            caps: HashMap::new(),
            caps_rx: Vec::new(),
            stacks: Vec::new(),
            stacked_with: None,
            open: HashSet::new(),
            tops: HashMap::new(),
            watch: None,
            settle: hot::Settle::default(),
            follow: self.settings.cull.follow_newest,
            astro: None,
        });
        if let Some(b) = self.cull.bench.as_mut() {
            b.step_at = Some(Instant::now());
        }
        self.chrome.set_presenting(true);
        self.chrome.announce_status(t!("cheatsheet-cull"));
        self.redraw = true;
    }

    /// Enters cull mode with the viewer's listing, or the gallery's (its order and filter).
    pub(super) fn cull_enter(&mut self) {
        if self.cull.run.is_some() || self.in_book() {
            return;
        }
        let from_gallery = self.gallery_active();
        let (listing, keep, focus) = if from_gallery {
            let (l, f) = self.gallery_cull_listing();
            (l, None, f)
        } else {
            let cur = self.cull_current_path();
            (self.entries.clone(), cur.clone(), cur)
        };
        if listing.is_empty() {
            // Say why nothing happened (the key is never greyed out: the gallery can cull).
            self.cull_say(t!("cull-nothing"), None);
            return;
        }
        self.slideshow_stop();
        self.gallery_marks_ready();
        let items = group::group(&listing, self.cull_show(), keep.as_deref());
        let at = focus
            .as_deref()
            .and_then(|f| group::item_of(&items, f))
            .unwrap_or(0);
        let shown = items[at].shown.clone();
        let dir = if from_gallery {
            self.gallery_dir().map(Path::to_path_buf)
        } else {
            self.folder.clone()
        };
        if from_gallery {
            self.gallery_hand_over(&shown);
        }
        self.cull_begin(listing, items, from_gallery, dir);
        let files = self.cull_rebuild(Some(&shown));
        self.cull_show_files(files, Some(&shown), at);
        self.cull_read_sidecars();
        self.cull_read_captures();
        self.cull_watch();
    }

    /// Groups the listing into items and the items into bursts (keeping `keep` on screen:
    /// a collapsed burst holding it shows it); the files to list, one per item shown.
    fn cull_rebuild(&mut self, keep: Option<&Path>) -> Vec<PathBuf> {
        let show = self.cull_show();
        let cfg = (self.settings.cull.bursts, self.settings.cull.burst_gap);
        let Some(mut run) = self.cull.run.take() else {
            return Vec::new();
        };
        run.show = show;
        run.items = group::group(&run.listing, show, keep);
        run.stacked_with = Some(cfg);
        let ranges = if cfg.0 {
            let caps: Vec<Option<Capture>> = run
                .items
                .iter()
                .map(|i| run.caps.get(&i.members[0]).copied().flatten())
                .collect();
            burst::bursts(&caps, (cfg.1 * 1000.0).round() as i64)
        } else {
            Vec::new()
        };
        let at = keep.and_then(|k| group::item_of(&run.items, k));
        let mut stacks = Vec::with_capacity(ranges.len());
        for r in ranges {
            let key = run.items[r.start].members[0].clone();
            let open = run.open.contains(&key);
            let kept = at.filter(|a| r.contains(a) && !open);
            let top = kept
                .or_else(|| {
                    run.tops
                        .get(&key)
                        .and_then(|t| group::item_of(&run.items, t))
                        .filter(|t| r.contains(t))
                })
                .unwrap_or_else(|| {
                    burst::top(r.clone(), |i| self.cull_mark_of(&run.items[i].members))
                });
            run.tops.insert(key, run.items[top].members[0].clone());
            stacks.push(Stack {
                items: r,
                top,
                open,
            });
        }
        run.stacks = stacks;
        let files = burst::visible(run.items.len(), &run.stacks)
            .into_iter()
            .map(|i| run.items[i].shown.clone())
            .collect();
        self.cull.run = Some(run);
        files
    }

    /// Lists `files` (from [`App::cull_rebuild`]) at the item holding `focus`, else at
    /// `fallback` (the next item when the current one left).
    fn cull_show_files(&mut self, files: Vec<PathBuf>, focus: Option<&Path>, fallback: usize) {
        if files.is_empty() {
            self.set_entries(files);
            self.current = None;
            self.shown = None;
            self.redraw = true;
            return;
        }
        let idx = focus
            .and_then(|f| {
                let run = self.cull.run.as_ref()?;
                let i = group::item_of(&run.items, f)?;
                files.iter().position(|p| *p == run.items[i].shown)
            })
            .unwrap_or(fallback.min(files.len() - 1));
        let same = self.cull_current_path().as_ref() == Some(&files[idx]);
        self.set_entries(files);
        if same {
            self.current = Some(idx);
            self.schedule();
        } else {
            self.go_to(idx);
        }
        self.redraw = true;
    }

    /// Items and bursts made again (the listing, the captures or the settings changed), at
    /// `focus` or the image on screen.
    fn cull_relayout(&mut self, focus: Option<PathBuf>) {
        let fallback = self.current.unwrap_or(0);
        let focus = focus.or_else(|| self.cull_current_path());
        let files = self.cull_rebuild(focus.as_deref());
        if files == self.entries && focus == self.cull_current_path() {
            self.redraw = true;
            return;
        }
        self.cull_show_files(files, focus.as_deref(), fallback);
    }

    /// Reads the captures of the items not read yet, in the background, a batch at a time.
    fn cull_read_captures(&mut self) {
        if !self.settings.cull.bursts {
            return;
        }
        let Some(run) = self.cull.run.as_mut() else {
            return;
        };
        let todo: Vec<Item> = run
            .items
            .iter()
            .filter(|i| !run.caps.contains_key(&i.members[0]))
            .cloned()
            .collect();
        if todo.is_empty() {
            return;
        }
        for i in &todo {
            run.caps.insert(i.members[0].clone(), None);
        }
        let (tx, rx) = crossbeam_channel::unbounded();
        run.caps_rx.push(rx);
        let wake = self.loader.waker();
        self.loader.background("cull-bursts", move |_| {
            for chunk in todo.chunks(256) {
                let v = chunk
                    .iter()
                    .map(|i| (i.members[0].clone(), burst::item_capture(i)))
                    .collect();
                if tx.send(v).is_err() {
                    return;
                }
                wake();
            }
        });
    }

    /// Watches the culled folder for new files while `cull.watch` is on.
    fn cull_watch(&mut self) {
        let want =
            self.settings.cull.watch && self.args.screenshot.is_none() && self.cull.bench.is_none();
        let wake = self.loader.waker();
        let Some(run) = self.cull.run.as_mut() else {
            return;
        };
        if !want {
            run.watch = None;
            run.settle = hot::Settle::default();
            return;
        }
        if run.watch.is_none()
            && let Some(d) = &run.dir
            && !self.cull.watch_failed.contains(d)
        {
            // Set up off this thread; a failure shows up later (see `cull_hot`).
            run.watch = Some(TreeWatch::start("cull-watch", d, 0, false, wake));
        }
    }

    /// Leaves cull mode at the current image: the full listing back, the window as it was,
    /// and with `to_gallery` the gallery it was entered from.
    pub(super) fn cull_exit(&mut self, to_gallery: bool) {
        let Some(run) = self.cull.run.take() else {
            return;
        };
        self.cull.bar = None;
        self.cull.hits.clear();
        self.cull.panel = None;
        self.cull.panel_hits.clear();
        self.science.blink = None;
        self.cull.hover = None;
        self.cull.tip_off = false;
        let cur = self.cull_current_path();
        if !run.listing.is_empty() {
            self.set_entries(run.listing);
            match cur.and_then(|p| self.index_of(&p)) {
                Some(i) => {
                    self.current = Some(i);
                    self.schedule();
                }
                None => self.go_to(0),
            }
        }
        if run.entered_fullscreen && self.fullscreen {
            self.toggle_fullscreen();
        }
        self.chrome.set_presenting(false);
        self.gallery_flush_marks(false);
        if run.from_gallery && to_gallery {
            self.gallery_enter();
        }
        self.scene = None;
        self.redraw = true;
    }

    /// Actions in cull mode; true: handled here.
    pub(super) fn cull_action(&mut self, a: Action) -> bool {
        if a == Action::ToggleCull {
            if self.cull.run.is_some() {
                self.cull_exit(true);
            } else {
                self.cull_enter();
            }
            return true;
        }
        if self.cull.run.is_none() {
            return false;
        }
        match a {
            Action::Back => {
                if self.chrome.back() || self.cancel_held_tool() || self.medical_back() {
                } else if self.shown.is_some() && !self.view.is_at_fit() {
                    self.view.zoom_fit();
                    self.after_view_change();
                } else {
                    self.cull_exit(true);
                }
                true
            }
            Action::Undo => {
                let paths = self.last_undo.and_then(|u| self.gallery_undo_paths(u));
                self.undo(None);
                if let Some(p) = paths.as_ref().and_then(|v| v.first()) {
                    self.cull_go_to_item_of(p);
                    self.cull_flash(t!("toast-undo"));
                }
                true
            }
            // The show would leave the items.
            Action::Slideshow => true,
            Action::ToggleGallery => {
                self.cull_exit(false);
                false
            }
            a => self.cull_mark_action(a),
        }
    }

    fn cull_go_to_item_of(&mut self, p: &Path) {
        let Some(run) = self.cull.run.as_ref() else {
            return;
        };
        let Some(i) = group::item_of(&run.items, p) else {
            return;
        };
        match self.index_of(&run.items[i].shown) {
            Some(idx) if self.current != Some(idx) => self.go_to(idx),
            Some(_) => {}
            // In a closed burst: it becomes the frame shown.
            None => self.cull_relayout(Some(p.to_path_buf())),
        }
    }

    /// In cull mode, the picked items' shown files (what Develop's sync and paste go to);
    /// `None` outside cull mode.
    pub(super) fn cull_picked(&self) -> Option<Vec<PathBuf>> {
        let run = self.cull.run.as_ref()?;
        Some(
            run.items
                .iter()
                .filter(|i| self.cull_mark_of(&i.members).flag == 1)
                .map(|i| i.shown.clone())
                .collect(),
        )
    }

    /// The photos the keyword panel acts on in cull mode (the current item's members);
    /// None when cull mode is off.
    pub(super) fn cull_keyword_targets(&self) -> Option<Vec<PathBuf>> {
        self.cull.run.as_ref()?;
        Some(self.cull_members())
    }

    /// The current item's members (the shown file alone if it is in no item).
    fn cull_members(&self) -> Vec<PathBuf> {
        let Some(cur) = self.cull_current_path() else {
            return Vec::new();
        };
        self.cull
            .run
            .as_ref()
            .and_then(|r| group::item_of(&r.items, &cur).map(|i| r.items[i].members.clone()))
            .unwrap_or_else(|| vec![cur])
    }

    /// An item's marks: its RAW's when set, else the first member's that is set.
    fn cull_mark_of(&self, members: &[PathBuf]) -> Mark {
        let marks: Vec<(bool, Mark)> = members
            .iter()
            .map(|p| (group::is_raw(p), self.gallery_mark_of(p)))
            .filter(|(_, m)| !m.is_empty())
            .collect();
        marks
            .iter()
            .find(|(raw, _)| *raw)
            .or(marks.first())
            .map(|(_, m)| *m)
            .unwrap_or_default()
    }

    /// Marks the current item (every member alike), confirms it, and moves on when `advance`
    /// and `cull.auto_advance`.
    fn cull_mark(
        &mut self,
        f: impl Fn(Mark) -> Mark,
        said: impl Fn(Mark) -> String,
        advance: bool,
    ) {
        let members = self.cull_members();
        if members.is_empty() {
            return;
        }
        let n = f(self.cull_mark_of(&members));
        self.gallery_set_marks(&members, |_| n);
        self.cull_flash(said(n));
        if advance && self.settings.cull.auto_advance {
            self.navigate(1);
        }
        self.redraw = true;
    }

    fn cull_flash(&mut self, msg: String) {
        let now = self.now();
        self.chrome.announce_status(msg.clone());
        if let Some(r) = self.cull.run.as_mut() {
            r.flash = Some((msg, now));
        }
        self.redraw = true;
    }

    /// The cull layer's own actions (by default `p`, `x`, `u`, `0`–`9` mark, `s` / `b` / `t`
    /// bursts and following new files, `Alt(+Shift)+1`–`3` the folder slots): true when one
    /// was run. Held keys do not repeat them (`Action::repeats`).
    fn cull_mark_action(&mut self, a: Action) -> bool {
        match a {
            Action::CullPick => {
                self.cull_mark(|m| Mark { flag: 1, ..m }, |_| t!("gallery-pick"), true)
            }
            Action::CullReject => {
                self.cull_mark(|m| Mark { flag: -1, ..m }, |_| t!("gallery-reject"), true)
            }
            Action::CullUnflag => {
                self.cull_mark(|m| Mark { flag: 0, ..m }, |_| t!("gallery-unflag"), false)
            }
            Action::CullRate(0) => self.cull_mark(
                |m| Mark { rating: 0, ..m },
                |_| t!("gallery-rate-none"),
                false,
            ),
            Action::CullRate(r) => self.cull_mark(
                |m| Mark { rating: r, ..m },
                |_| t!("gallery-stars", stars = i64::from(r)),
                true,
            ),
            // Red, yellow, green, blue; again takes it off.
            Action::CullLabel(l) => self.cull_mark(
                |m| Mark {
                    label: if m.label == l { 0 } else { l },
                    ..m
                },
                |m| mark_label_name(m.label),
                false,
            ),
            Action::CullBurst => self.cull_toggle_stack(),
            Action::CullKeepBest => self.cull_keep_best(),
            Action::CullFollow => self.cull_toggle_follow(),
            Action::CullSubframes
            | Action::CullSortNext
            | Action::CullSortReverse
            | Action::CullMetricNext
            | Action::CullSigmaDown
            | Action::CullSigmaUp
            | Action::CullAutoReject
            | Action::CullMoveRejects
            | Action::CullSameStretch => self.cull_astro_action(a),
            Action::CullCopyTo(n) => self.cull_slot(n.into(), true),
            Action::CullMoveTo(n) => self.cull_slot(n.into(), false),
            _ => return false,
        }
        true
    }

    /// A click on the strip (logical px); true when it was over the strip.
    pub(super) fn cull_click(&mut self, p: (f32, f32), down: bool) -> bool {
        if self.cull_panel_click(p, down) {
            return true;
        }
        if self.cull.run.is_none() || !self.cull.bar.is_some_and(|b| b.contains(p)) {
            return false;
        }
        let hit = self
            .cull
            .hits
            .iter()
            .find(|(r, _)| r.contains(p))
            .map(|h| h.1);
        if down && let Some(h) = hit {
            self.cull_hit(h);
        }
        true
    }

    fn cull_hit(&mut self, h: Hit) {
        match h {
            Hit::Star(n) => self.cull_mark(
                |m| Mark { rating: n, ..m },
                |_| t!("gallery-stars", stars = i64::from(n)),
                false,
            ),
            // A click on a set flag takes it off.
            Hit::Pick | Hit::Reject => {
                let f = if h == Hit::Pick { 1 } else { -1 };
                self.cull_mark(
                    |m| Mark {
                        flag: if m.flag == f { 0 } else { f },
                        ..m
                    },
                    |m| match m.flag {
                        1 => t!("gallery-pick"),
                        -1 => t!("gallery-reject"),
                        _ => t!("gallery-unflag"),
                    },
                    false,
                );
            }
            Hit::Exit => self.cull_exit(true),
            Hit::Stack => self.cull_toggle_stack(),
            Hit::Label => {}
        }
    }

    /// Whether `p` (logical px) is over the strip as last drawn: the pointer never hides
    /// there.
    pub(super) fn cull_over_strip(&self, p: (f32, f32)) -> bool {
        self.cull.run.is_some() && self.cull.bar.is_some_and(|b| b.contains(p))
    }

    /// The pointer moved to `p` (logical px) or left the window (`None`): the control under
    /// it, for the hover look, the tooltip and the hand pointer.
    pub(super) fn cull_pointer_move(&mut self, p: Option<(f32, f32)>) {
        let now = self.now();
        if self.cull.hover_at(p, now) {
            self.redraw = true;
        }
    }

    /// A click or key press puts the tooltip away.
    pub(super) fn cull_dismiss_tip(&mut self) {
        if self.cull.hover.is_some() && !self.cull.tip_off {
            self.cull.tip_off = true;
            self.redraw = true;
        }
    }

    /// The burst holding the current item, and the item.
    fn cull_stack_here(&self) -> Option<(usize, usize)> {
        let run = self.cull.run.as_ref()?;
        let cur = self.cull_current_path()?;
        let i = group::item_of(&run.items, &cur)?;
        Some((burst::stack_of(&run.stacks, i)?, i))
    }

    /// `s`: opens the burst on screen, or closes it on the frame on screen.
    fn cull_toggle_stack(&mut self) {
        let Some((s, i)) = self.cull_stack_here() else {
            self.cull_flash(t!("cull-burst-none"));
            return;
        };
        let Some(run) = self.cull.run.as_mut() else {
            return;
        };
        let st = run.stacks[s].clone();
        let key = run.items[st.items.start].members[0].clone();
        let n = st.items.len() as i64;
        let msg = if st.open {
            run.open.remove(&key);
            run.tops.insert(key, run.items[i].members[0].clone());
            t!("cull-burst-closed", count = n)
        } else {
            run.open.insert(key);
            t!(
                "cull-burst-open",
                index = (i - st.items.start + 1) as i64,
                count = n
            )
        };
        self.cull_relayout(None);
        self.cull_flash(msg);
    }

    /// `b`: picks the frame on screen, rejects the rest of its burst (one undo), and closes
    /// the burst on it.
    fn cull_keep_best(&mut self) {
        let Some((s, i)) = self.cull_stack_here() else {
            self.cull_flash(t!("cull-burst-none"));
            return;
        };
        let Some(run) = self.cull.run.as_ref() else {
            return;
        };
        let st = run.stacks[s].clone();
        let items: Vec<(usize, Vec<PathBuf>)> = st
            .items
            .clone()
            .map(|j| (j, run.items[j].members.clone()))
            .collect();
        let key = run.items[st.items.start].members[0].clone();
        let best = run.items[i].members[0].clone();
        let mut changes = Vec::new();
        for (j, members) in &items {
            let m = Mark {
                flag: if *j == i { 1 } else { -1 },
                ..self.cull_mark_of(members)
            };
            changes.extend(members.iter().map(|p| (p.clone(), m)));
        }
        self.gallery_set_marks_each(&changes);
        if let Some(run) = self.cull.run.as_mut() {
            run.open.remove(&key);
            run.tops.insert(key, best);
        }
        self.cull_relayout(None);
        self.cull_flash(t!("cull-burst-best", count = (items.len() - 1) as i64));
        if self.settings.cull.auto_advance {
            self.navigate(1);
        }
    }

    /// `t`: follows new files as they arrive, or stops.
    fn cull_toggle_follow(&mut self) {
        let Some(run) = self.cull.run.as_mut() else {
            return;
        };
        run.follow = !run.follow;
        let msg = if run.follow {
            t!("cull-follow-on")
        } else {
            t!("cull-follow-off")
        };
        self.cull_flash(msg);
    }

    /// Alt+1–3 (`copy`) or Alt+Shift+1–3: the current item, its sidecars with it, to the
    /// folder of slot `slot`, in the background.
    fn cull_slot(&mut self, slot: usize, copy: bool) {
        let c = &self.settings.cull;
        let dest = match slot {
            1 => c.slot_1.clone(),
            2 => c.slot_2.clone(),
            _ => c.slot_3.clone(),
        };
        let Some(dest) = dest else {
            self.cull_flash(t!("cull-slot-unset", slot = slot as i64));
            return;
        };
        if let Err(e) = slots::check_dest(&dest) {
            self.cull_flash(t!(
                "cull-slot-missing",
                slot = slot as i64,
                reason = crate::error_text::io_reason(&e)
            ));
            return;
        }
        let collision = match c.collisions {
            CullCollision::Skip => Collision::Skip,
            _ => Collision::Suffix,
        };
        let members = self.cull_members();
        // An item under way already stays with that.
        if members.is_empty() || members.iter().any(|m| self.cull.busy.contains(m)) {
            return;
        }
        let stem = self
            .cull_current_path()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_default();
        let tx = self
            .cull
            .ops
            .get_or_insert_with(crossbeam_channel::unbounded)
            .0
            .clone();
        self.cull.busy.extend(members.iter().cloned());
        self.cull.ops_running += 1;
        let wake = self.loader.waker();
        self.loader.background("cull-slot", move |_| {
            let files = slots::with_sidecars(&members);
            let outcome = slots::place(&files, &stem, &dest, copy, collision);
            let _ = tx.send(Op::Placed {
                copy,
                folder: dest,
                outcome,
                members,
            });
            wake();
        });
    }

    /// Undo of a copy or move to a slot; false when `u` is not one.
    pub(super) fn cull_undo(&mut self, u: u64) -> bool {
        let Some(op) = self.cull.undo.remove(&u) else {
            return false;
        };
        self.chrome.dismiss_undo(u);
        if self.last_undo == Some(u) {
            self.last_undo = None;
        }
        let tx = self
            .cull
            .ops
            .get_or_insert_with(crossbeam_channel::unbounded)
            .0
            .clone();
        self.cull.ops_running += 1;
        let wake = self.loader.waker();
        self.loader.background("cull-slot-undo", move |_| {
            let SlotUndo { copy, placed, at } = op;
            let (done, kept, failed) = if copy {
                slots::remove_copies(&placed)
            } else {
                let (done, failed) = slots::put_back(&placed);
                (done, Vec::new(), failed)
            };
            let _ = tx.send(Op::Undone {
                copy,
                at,
                placed,
                done,
                kept,
                failed,
            });
            wake();
        });
        true
    }

    /// File operations finished: the listing, the marks and the undo follow.
    fn cull_ops_done(&mut self) {
        let Some((_, rx)) = &self.cull.ops else {
            return;
        };
        let done: Vec<Op> = rx.try_iter().collect();
        for op in done {
            self.cull.ops_running = self.cull.ops_running.saturating_sub(1);
            match op {
                Op::Placed {
                    copy,
                    folder,
                    outcome,
                    members,
                } => self.cull_placed(copy, &folder, outcome, &members),
                Op::Rejects { folder, outcomes } => self.cull_rejects_moved(&folder, outcomes),
                Op::Undone {
                    copy,
                    at,
                    placed,
                    done,
                    kept,
                    failed,
                } => self.cull_undone(copy, at, &placed, done, &kept, &failed),
            }
        }
    }

    fn cull_say(&mut self, msg: String, undo: Option<u64>) {
        let now = self.now();
        self.chrome.announce_status(msg.clone());
        self.chrome.toast(msg, undo, now);
        self.redraw = true;
    }

    /// Says what failed (the first few): `say(file name, reason)`.
    fn cull_failures(
        &mut self,
        failed: &[(PathBuf, String)],
        say: impl Fn(String, String) -> String,
    ) {
        for (p, e) in failed.iter().take(3) {
            let file = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            self.cull_say(say(file, e.clone()), None);
        }
    }

    fn cull_placed(&mut self, copy: bool, folder: &Path, outcome: Outcome, members: &[PathBuf]) {
        for m in members {
            self.cull.busy.remove(m);
        }
        let folder_name = folder
            .file_name()
            .unwrap_or(folder.as_os_str())
            .to_string_lossy()
            .into_owned();
        if outcome.same_folder {
            self.cull_flash(t!("cull-slot-same", folder = folder_name));
            return;
        }
        if outcome.skipped {
            let name = members
                .first()
                .and_then(|m| m.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            self.cull_flash(t!("cull-slot-skipped", folder = folder_name, name = name));
            return;
        }
        let op = if copy { "copy" } else { "move" };
        self.cull_failures(&outcome.failed, |file, reason| {
            t!("cull-slot-failed", op = op, file = file, reason = reason)
        });
        if outcome.done.is_empty() {
            return;
        }
        let pairs: Vec<(PathBuf, PathBuf)> = outcome
            .done
            .iter()
            .map(|p| (p.from.clone(), p.to.clone()))
            .collect();
        self.gallery_marks_follow(&pairs, copy);
        let mut at = 0;
        if let Some(run) = self.cull.run.as_mut() {
            at = run
                .listing
                .iter()
                .position(|p| members.contains(p))
                .unwrap_or(run.listing.len());
            if !copy {
                let gone: HashSet<&PathBuf> = pairs.iter().map(|(f, _)| f).collect();
                run.listing.retain(|p| !gone.contains(p));
            }
        }
        if !copy && self.cull.run.is_some() {
            self.cull_relayout(None);
        }
        let u = self.next_undo;
        self.next_undo += 1;
        self.last_undo = Some(u);
        let msg = match &outcome.renamed {
            Some(n) => t!("cull-slot-renamed", name = n.clone(), folder = folder_name),
            None if copy => t!("cull-copied", folder = folder_name),
            None => t!("cull-moved", folder = folder_name),
        };
        self.cull.undo.insert(
            u,
            SlotUndo {
                copy,
                placed: outcome.done,
                at,
            },
        );
        self.cull_say(msg, Some(u));
    }

    fn cull_undone(
        &mut self,
        copy: bool,
        at: usize,
        placed: &[Placed],
        done: Vec<PathBuf>,
        kept: &[PathBuf],
        failed: &[(PathBuf, String)],
    ) {
        if copy {
            self.gallery_marks_forget(&done);
        } else {
            let pairs: Vec<(PathBuf, PathBuf)> = placed
                .iter()
                .filter(|p| done.contains(&p.from))
                .map(|p| (p.to.clone(), p.from.clone()))
                .collect();
            self.gallery_marks_follow(&pairs, false);
            let images: Vec<PathBuf> = done
                .into_iter()
                .filter(|p| self.loader.supports_file(p))
                .collect();
            if let Some(run) = self.cull.run.as_mut()
                && !images.is_empty()
            {
                let at = at.min(run.listing.len());
                let back: Vec<PathBuf> = images
                    .iter()
                    .filter(|p| !run.listing.contains(p))
                    .cloned()
                    .collect();
                run.listing.splice(at..at, back);
                self.cull_relayout(images.first().cloned());
            }
        }
        if !kept.is_empty() {
            self.cull_say(t!("cull-copy-kept", count = kept.len() as i64), None);
        }
        self.cull_failures(failed, |file, reason| {
            t!("cull-undo-failed", file = file, reason = reason)
        });
        if failed.is_empty() {
            self.cull_flash(t!("toast-undo"));
        }
    }

    /// New files in the culled folder: noted when the watch reports them, and added to the
    /// run (at the end) once they stopped changing.
    fn cull_hot(&mut self) {
        let now = Instant::now();
        let Some(run) = self.cull.run.as_mut() else {
            return;
        };
        if let Some(w) = &run.watch
            && w.failed()
        {
            debug!(target: UI, "cull: no watch on {}", w.dir().display());
            self.cull.watch_failed.insert(w.dir().to_path_buf());
            run.watch = None;
        }
        if let Some(w) = &run.watch
            && let Some(ch) = w.take(now)
        {
            let listed: HashSet<&PathBuf> = run.listing.iter().collect();
            for p in ch.paths {
                let fresh = p.parent() == run.dir.as_deref()
                    && !p
                        .file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with('.'))
                    && !listed.contains(&p)
                    && self.loader.supports_file(&p);
                if fresh {
                    run.settle.saw(p, now);
                }
            }
        }
        let ready = run.settle.poll(now, hot::stat);
        self.cull_arrived(ready);
    }

    /// The folder being culled.
    pub(super) fn cull_dir(&self) -> Option<&Path> {
        self.cull.run.as_ref()?.dir.as_deref()
    }

    /// Follows new files from now on (a tethered camera connected).
    pub(super) fn cull_follow_on(&mut self) {
        if let Some(run) = self.cull.run.as_mut() {
            run.follow = true;
        }
    }

    /// Files that arrived in the culled folder (from the watch, or tethered capture) join
    /// the run at the end; shown when following.
    pub(super) fn cull_arrived(&mut self, ready: Vec<PathBuf>) {
        let Some(run) = self.cull.run.as_mut() else {
            return;
        };
        let new: Vec<PathBuf> = ready
            .into_iter()
            .filter(|p| !run.listing.contains(p))
            .collect();
        if new.is_empty() {
            return;
        }
        run.listing.extend(new.iter().cloned());
        // Develop's next capture style, before the new files are shown.
        self.develop_next_capture(&new);
        let Some(run) = self.cull.run.as_mut() else {
            return;
        };
        let follow = run.follow;
        let n = new.len() as i64;
        debug!(target: UI, "cull: {n} new files");
        self.cull_relayout(if follow { new.last().cloned() } else { None });
        self.cull_read_captures();
        if !follow {
            self.cull_flash(t!("cull-new-files", count = n));
        }
    }

    /// A screen reader's click on one of the strip's buttons.
    pub(super) fn cull_a11y_action(&mut self, target: u64) -> bool {
        if self.cull.run.is_none() || target <= A11Y_BASE {
            return false;
        }
        if target > astro::A11Y_PANEL {
            let Some(&(_, h)) = self
                .cull
                .panel_hits
                .get((target - astro::A11Y_PANEL - 1) as usize)
            else {
                return false;
            };
            self.cull_panel_hit(h);
            return true;
        }
        let Some(&(_, h)) = self.cull.hits.get((target - A11Y_BASE - 1) as usize) else {
            return false;
        };
        self.cull_hit(h);
        true
    }

    /// The strip's contents this frame, in cull mode.
    pub(super) fn cull_strip(&self, now: f64) -> Option<Strip> {
        let run = self.cull.run.as_ref()?;
        let cur = self.cull_current_path()?;
        let at = group::item_of(&run.items, &cur);
        let title = match at {
            Some(i) => group::caption(&run.items[i]),
            None => cur
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        };
        let exposure = self.cull_astro_line(&cur).or_else(|| {
            self.current_id()
                .and_then(|id| self.images.get(&id))
                .and_then(|i| i.metadata.as_deref())
                .and_then(|m| m.exposure_line.clone())
        });
        let reduce = self.reduce_motion();
        let flash = run.flash.as_ref().and_then(|(msg, t0)| {
            let age = now - t0;
            (age < FLASH_S).then(|| {
                let a = if reduce {
                    1.0
                } else {
                    ((FLASH_S - age) / FLASH_FADE_S).min(1.0) as f32
                };
                (msg.clone(), a)
            })
        });
        let burst = at.and_then(|i| {
            let s = &run.stacks[burst::stack_of(&run.stacks, i)?];
            Some((i - s.items.start + 1, s.items.len(), s.open))
        });
        let mut strip = Strip {
            title,
            position: (self.current.map_or(0, |i| i + 1), self.entries.len()),
            exposure,
            flags: {
                let mut f = self.cull_flags(&cur);
                f.extend(self.develop_cull_flags(&self.cull_members()));
                f
            },
            mark: self.cull_mark_of(&self.cull_members()),
            burst,
            flash,
            hover: self.cull.hovered(),
            tip: None,
            pal: self.palette(),
        };
        strip.tip = self
            .cull
            .tip_due(now)
            .ok()
            .map(|h| tip_text(h, &strip, &self.keymap));
        Some(strip)
    }

    /// When the confirmation next changes (s), or the benchmark's pace.
    pub(super) fn cull_wake(&self, now: f64) -> Option<f64> {
        let run = self.cull.run.as_ref()?;
        let mut wake = f64::INFINITY;
        if let Some((_, t0)) = &run.flash {
            let age = now - t0;
            if age < FLASH_S {
                let fade_from = FLASH_S - FLASH_FADE_S;
                wake = if self.reduce_motion() {
                    FLASH_S - age
                } else if age >= fade_from {
                    0.0
                } else {
                    fade_from - age
                };
            }
        }
        if run.sidecars.is_some()
            || !run.caps_rx.is_empty()
            || self.cull.ops_running > 0
            || run.astro.as_ref().is_some_and(|a| !a.rx.is_empty())
        {
            wake = wake.min(0.1);
        }
        let watch = run.watch.as_ref().and_then(TreeWatch::due);
        if let Some(t) = watch.into_iter().chain(run.settle.due()).min() {
            let left = t.saturating_duration_since(Instant::now()).as_secs_f64();
            wake = wake.min(left);
        }
        if let Err(wait) = self.cull.tip_due(now) {
            wake = wake.min(wait);
        }
        if self.cull.bench.is_some() {
            wake = wake.min(0.005);
        }
        wake.is_finite().then_some(wake)
    }

    /// Reads the items' sidecars in the background (the marks table wins over them).
    fn cull_read_sidecars(&mut self) {
        if !self.settings.gallery.rating_sidecars || self.args.screenshot.is_some() {
            return;
        }
        let prefer = self.gallery_prefer_folders();
        let Some(run) = self.cull.run.as_mut() else {
            return;
        };
        let paths = run.listing.clone();
        let (tx, rx) = crossbeam_channel::bounded(1);
        run.sidecars = Some(rx);
        let wake = self.loader.waker();
        self.loader.background("cull-sidecars", move |_| {
            let _ = tx.send(marks::read_sidecars(paths, &prefer, &|| false));
            wake();
        });
    }

    /// From `about_to_wait`: sidecar marks in, marks written when due, `cull.show` followed,
    /// and the benchmark's steps.
    pub(super) fn cull_tick(&mut self, el: &ActiveEventLoop) {
        self.cull_ops_done();
        if self.cull.run.is_none() {
            return;
        }
        self.gallery_flush_marks(false);
        if let Some(run) = self.cull.run.as_mut()
            && let Some(rx) = &run.sidecars
            && let Ok(m) = rx.try_recv()
        {
            run.sidecars = None;
            self.gallery_merge_sidecar_marks(m);
            self.redraw = true;
        }
        // Captures in: the bursts follow.
        let mut restack = false;
        if let Some(run) = self.cull.run.as_mut() {
            let caps = &mut run.caps;
            run.caps_rx.retain(|rx| {
                loop {
                    match rx.try_recv() {
                        Ok(v) => {
                            restack = true;
                            caps.extend(v);
                        }
                        Err(crossbeam_channel::TryRecvError::Empty) => break true,
                        Err(crossbeam_channel::TryRecvError::Disconnected) => break false,
                    }
                }
            });
        }
        // `cull.show` or the burst settings changed.
        let show = self.cull_show();
        let cfg = (self.settings.cull.bursts, self.settings.cull.burst_gap);
        let (show_changed, cfg_changed) = self.cull.run.as_ref().map_or((false, false), |r| {
            (r.show != show, r.stacked_with != Some(cfg))
        });
        if show_changed {
            // The member on screen changes with it.
            let cur = self.cull_current_path();
            let item = cur.and_then(|p| {
                let run = self.cull.run.as_ref()?;
                group::item_of(&run.items, &p).map(|i| run.items[i].members.clone())
            });
            let fallback = self.current.unwrap_or(0);
            let files = self.cull_rebuild(None);
            let focus = item.and_then(|m| {
                let run = self.cull.run.as_ref()?;
                run.items
                    .iter()
                    .find(|i| i.members == m)
                    .map(|i| i.shown.clone())
            });
            self.cull_show_files(files, focus.as_deref(), fallback);
        } else if restack || cfg_changed {
            self.cull_relayout(None);
        }
        if cfg_changed {
            self.cull_read_captures();
        }
        self.cull_watch();
        self.cull_hot();
        self.cull_astro_tick();
        self.cull_bench_tick(el);
    }

    /// The current image is sharp (from the frame that showed it).
    pub(super) fn cull_sharp(&mut self) {
        let at = self
            .current_id()
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.shown_at);
        if let Some(b) = self.cull.bench.as_mut()
            && b.sharp_ms.is_none()
            && let Some(t) = at
        {
            b.sharp_ms = Some(t.elapsed().as_secs_f64() * 1000.0);
        }
    }

    fn cull_bench_tick(&mut self, el: &ActiveEventLoop) {
        let img = self.current_id().and_then(|id| self.images.get(&id));
        let open_ms = img.and_then(|i| i.open_ms);
        // Sharp already (a prefetched image, or one upgraded at upload): this tick bounds it.
        let sharp_now = img
            .filter(|i| i.state == "sharp")
            .and_then(|i| i.shown_at)
            .map(|t| t.elapsed().as_secs_f64() * 1000.0);
        let len = self.entries.len();
        let Some(b) = self.cull.bench.as_mut() else {
            return;
        };
        let Some(at) = b.step_at else { return };
        if b.sharp_ms.is_none() {
            b.sharp_ms = sharp_now;
        }
        if let (Some(o), Some(s)) = (open_ms, b.sharp_ms.as_mut()) {
            *s = s.max(o);
        }
        let waited = at.elapsed().as_secs_f64();
        let (Some(open), Some(sharp)) = (open_ms, b.sharp_ms) else {
            if waited > 30.0 {
                eprintln!("cull bench: an image did not get sharp in 30 s");
                el.exit();
            }
            return;
        };
        if waited < b.interval {
            return;
        }
        if b.first.is_none() {
            b.first = Some((open, sharp));
        } else {
            b.open.push(open);
            b.sharp.push(sharp);
        }
        if b.open.len() >= b.steps || self.current.is_some_and(|c| c + 1 >= len) {
            let (f_open, f_sharp) = b.first.unwrap_or_default();
            let n = b.open.len();
            println!(
                "{{\"cull_bench\": {{\"items\": {len}, \"steps\": {n}, \"interval_ms\": {:.0}, \
                 \"first\": {{\"open_ms\": {f_open:.1}, \"sharp_ms\": {f_sharp:.1}}}, \
                 \"open_ms\": {{\"p50\": {:.1}, \"p95\": {:.1}, \"max\": {:.1}}}, \
                 \"sharp_ms\": {{\"p50\": {:.1}, \"p95\": {:.1}, \"max\": {:.1}}}}}}}",
                b.interval * 1000.0,
                pct(&mut b.open, 0.5),
                pct(&mut b.open, 0.95),
                pct(&mut b.open, 1.0),
                pct(&mut b.sharp, 0.5),
                pct(&mut b.sharp, 0.95),
                pct(&mut b.sharp, 1.0),
            );
            self.cull.bench = None;
            el.exit();
            return;
        }
        b.sharp_ms = None;
        b.step_at = Some(Instant::now());
        self.navigate(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic files only, in the build's scratch folder (`SPECULAR_TEST_TMP` overrides).
    fn scratch(name: &str) -> PathBuf {
        let base = std::env::var_os("SPECULAR_TEST_TMP").map_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/test-tmp"),
            PathBuf::from,
        );
        let d = base.join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A mark on a RAW+JPEG item lands in both members' sidecars: the stem `.xmp` is the
    /// RAW's, the JPEG's is the long form; the images are left as they were.
    #[test]
    fn a_mark_on_a_pair_reaches_both_sidecars() {
        let d = scratch("cull-pair");
        let bytes = b"not really an image";
        for f in ["IMG_7.CR3", "IMG_7.JPG", "IMG_8.JPG"] {
            std::fs::write(d.join(f), bytes).unwrap();
        }
        let listing: Vec<PathBuf> = ["IMG_7.CR3", "IMG_7.JPG", "IMG_8.JPG"]
            .iter()
            .map(|f| d.join(f))
            .collect();
        let items = group::group(&listing, Show::Jpeg, None);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].shown, d.join("IMG_7.JPG"));
        let m = Mark {
            rating: 4,
            label: 2,
            flag: 1,
        };
        let changes = items[0].members.iter().map(|p| (p.clone(), m)).collect();
        assert!(marks::write_sidecars(marks::sidecar_ticket(), changes).is_empty());
        let read = |f: &str| marks::parse_xmp(&std::fs::read_to_string(d.join(f)).unwrap());
        assert_eq!(read("IMG_7.xmp"), m);
        assert_eq!(read("IMG_7.JPG.xmp"), m);
        assert!(!d.join("IMG_8.xmp").exists());
        for f in ["IMG_7.CR3", "IMG_7.JPG"] {
            assert_eq!(std::fs::read(d.join(f)).unwrap(), bytes);
        }
        let back: std::collections::HashMap<_, _> =
            marks::read_sidecars(listing.clone(), &[], &|| false)
                .into_iter()
                .collect();
        assert_eq!(back.get(&d.join("IMG_7.CR3")), Some(&m));
        assert_eq!(back.get(&d.join("IMG_7.JPG")), Some(&m));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn strip_label_reads_every_part() {
        let s = Strip {
            title: "IMG_7 · JPG + CR3".into(),
            position: (3, 12),
            exposure: Some("1/250 s · f/8 · ISO 100 · 35 mm".into()),
            flags: Vec::new(),
            mark: Mark {
                rating: 3,
                label: 0,
                flag: -1,
            },
            burst: Some((2, 5, true)),
            flash: None,
            hover: None,
            tip: None,
            pal: Palette::dark(),
        };
        let burst = s.burst_text().unwrap();
        assert!(burst.contains('2') && burst.contains('5'), "{burst}");
        assert_eq!(
            s.detail(),
            format!("3 / 12 · {burst} · 1/250 s · f/8 · ISO 100 · 35 mm")
        );
        let l = s.label();
        assert!(l.contains(&burst), "{l}");
        assert!(l.starts_with("IMG_7 · JPG + CR3, "), "{l}");
        assert!(l.contains("1/250 s"), "{l}");
        assert_eq!(mark_words(s.mark).len(), 2);
        assert!(mark_words(Mark::default()).is_empty());
    }

    fn running() -> Cull {
        let mut c = Cull {
            run: Some(Run {
                listing: Vec::new(),
                items: Vec::new(),
                show: Show::Jpeg,
                entered_fullscreen: false,
                from_gallery: false,
                flash: None,
                sidecars: None,
                dir: None,
                caps: HashMap::new(),
                caps_rx: Vec::new(),
                stacks: Vec::new(),
                stacked_with: None,
                open: HashSet::new(),
                tops: HashMap::new(),
                watch: None,
                settle: hot::Settle::default(),
                follow: false,
                astro: None,
            }),
            ..Cull::default()
        };
        c.set_hits(
            Rect::new(0.0, 400.0, 720.0, 50.0),
            vec![
                (Rect::new(500.0, 410.0, 30.0, 30.0), Hit::Star(1)),
                (Rect::new(530.0, 410.0, 30.0, 30.0), Hit::Star(2)),
                (Rect::new(600.0, 410.0, 30.0, 30.0), Hit::Label),
            ],
        );
        c
    }

    /// The control under the pointer follows it, changes only when it does, and clears when
    /// the pointer leaves the controls, the window or cull mode.
    #[test]
    fn hover_follows_the_pointer_and_clears() {
        let mut c = running();
        assert!(c.hover_at(Some((510.0, 420.0)), 1.0));
        assert_eq!(c.hovered(), Some(Hit::Star(1)));
        assert!(c.over_button());
        // Moving within the same star keeps when it started (the tooltip's wait).
        assert!(!c.hover_at(Some((515.0, 425.0)), 1.2));
        assert_eq!(c.hover, Some((Hit::Star(1), 1.0)));
        assert!(c.hover_at(Some((540.0, 420.0)), 1.3));
        assert_eq!(c.hovered(), Some(Hit::Star(2)));
        // The label has a tooltip but is no button: no hand there.
        assert!(c.hover_at(Some((610.0, 420.0)), 1.4));
        assert_eq!(c.hovered(), Some(Hit::Label));
        assert!(!c.over_button());
        // Between the controls, on the strip.
        assert!(c.hover_at(Some((100.0, 420.0)), 1.5));
        assert_eq!(c.hovered(), None);
        c.hover_at(Some((510.0, 420.0)), 1.6);
        assert!(c.hover_at(None, 1.7), "leaving the window clears it");
        assert_eq!(c.hovered(), None);
        c.hover_at(Some((510.0, 420.0)), 1.8);
        c.run = None;
        assert!(c.hover_at(Some((510.0, 420.0)), 1.9), "out of cull mode");
        assert_eq!(c.hovered(), None);
        assert!(!c.over_button());
    }

    /// The tooltip comes after the standard delay, goes on a click or key press, and comes
    /// back for the next control.
    #[test]
    fn tooltip_waits_and_dismisses() {
        let delay = f64::from(durations::TOOLTIP_DELAY);
        let mut c = running();
        assert_eq!(c.tip_due(0.0), Err(f64::INFINITY));
        c.hover_at(Some((510.0, 420.0)), 1.0);
        let Err(wait) = c.tip_due(1.1) else {
            panic!("tooltip before the delay")
        };
        assert!((wait - (delay - 0.1)).abs() < 1e-9);
        assert_eq!(c.tip_due(1.0 + delay), Ok(Hit::Star(1)));
        c.tip_off = true;
        assert_eq!(c.tip_due(5.0), Err(f64::INFINITY));
        c.hover_at(Some((540.0, 420.0)), 6.0);
        assert_eq!(c.tip_due(6.0 + delay), Ok(Hit::Star(2)));
    }

    /// Each tooltip names the control and its key; leaving takes the keymap's key.
    #[test]
    fn tooltips_name_the_control_and_its_key() {
        let mut s = Strip {
            title: String::new(),
            position: (1, 1),
            exposure: None,
            flags: Vec::new(),
            mark: Mark {
                rating: 0,
                label: 3,
                flag: 0,
            },
            burst: Some((1, 4, false)),
            flash: None,
            hover: None,
            tip: None,
            pal: Palette::dark(),
        };
        let mut km = specular_input::Keymap::default();
        let key = |k: &str| format!("({k})");
        // Fluent wraps arguments in directional isolates; compare without them.
        let plain = |s: String| s.replace(['\u{2068}', '\u{2069}'], "");
        let tip = |h: Hit, s: &Strip, km: &specular_input::Keymap| plain(tip_text(h, s, km));
        let star = tip(Hit::Star(3), &s, &km);
        assert!(
            star.starts_with(&plain(t!("gallery-stars", stars = 3))),
            "{star}"
        );
        assert!(star.ends_with(&key("3")), "{star}");
        assert!(tip(Hit::Pick, &s, &km).ends_with(&key("P")));
        assert!(tip(Hit::Reject, &s, &km).ends_with(&key("X")));
        let open = tip(Hit::Stack, &s, &km);
        assert!(open.starts_with(&plain(t!("cull-tip-burst-open"))) && open.ends_with(&key("S")));
        s.burst = Some((1, 4, true));
        assert!(tip(Hit::Stack, &s, &km).starts_with(&plain(t!("cull-tip-burst-close"))));
        let label = tip(Hit::Label, &s, &km);
        assert!(label.contains(&plain(mark_label_name(3))), "{label}");
        assert!(label.ends_with(&key("6–9")), "{label}");
        let exit = tip(Hit::Exit, &s, &km);
        let bound = km.primary_key(Action::ToggleCull).unwrap().to_string();
        assert!(exit.ends_with(&key(&bound)), "{exit}");
        // Remapped: the tooltip follows.
        use specular_input::{Key, KeyCombo, KeymapEdit, Mods};
        let f9 = KeyCombo::new(Key::Named(specular_input::NamedKey::F(9)), Mods::NONE);
        for k in km.keys_for(Action::ToggleCull) {
            km.unbind(k.first());
        }
        km.bind(f9, Action::ToggleCull);
        let exit = tip(Hit::Exit, &s, &km);
        assert!(exit.ends_with(&key(&f9.to_string())), "{exit}");
        // A cull key rebound in Settings → Keys: Pick moves from `p` to `k`.
        let unbind = |km: &mut specular_input::Keymap, a: Action| {
            for keys in km.keys_in(Mode::Cull, a) {
                km.apply_edit(&KeymapEdit::Unbind {
                    mode: Mode::Cull,
                    keys,
                });
            }
        };
        unbind(&mut km, Action::CullPick);
        km.bind(KeyCombo::parse("k").unwrap(), Action::CullPick);
        let pick = tip(Hit::Pick, &s, &km);
        assert!(pick.ends_with(&key("K")), "{pick}");
        // Unbound: no key at all.
        unbind(&mut km, Action::CullPick);
        assert_eq!(tip(Hit::Pick, &s, &km), plain(t!("gallery-pick")));
    }
}
