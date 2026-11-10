//! The first-run question, "What will you use Specular for?": what a choice does, a check box
//! per profile with what it is for (General always on), a note that Settings › Profile changes it
//! later, and Continue, which stores the choice in `profile.capabilities`. Beneath the profiles,
//! what a start without a file shows (`general.on_start`, three radio buttons). Skip (and Esc)
//! closes it keeping every default.

use specular_gfx::{Icon, Rect, draw_icon};
use specular_i18n::{I18n, t};
use specular_input::{Capabilities, Capability};
use specular_settings::{OnStart, Origin, ProfileCapability, SettingEnum};
use specular_theme::metrics;

use crate::ctx::{Ctx, HAlign, Target};
use crate::{Chrome, Modal, Response, ViewerState};

/// The label of one profile's check box (the Settings option's).
pub(crate) fn label(c: Capability) -> String {
    I18n::global().tr(&format!("settings-profile-capabilities-{}", c.id()))
}

/// What one profile is for, under its check box.
pub(crate) fn about(c: Capability) -> String {
    t!("profile-about", capability = c.id())
}

/// The start choices, in order.
pub(crate) const STARTS: [OnStart; 3] = [OnStart::StartScreen, OnStart::Gallery, OnStart::Library];

/// The label of a start choice (the Settings option's).
pub(crate) fn start_label(o: OnStart) -> String {
    I18n::global().tr(&format!(
        "settings-general-on-start-{}",
        o.as_str().replace('_', "-")
    ))
}

/// The profiles a check box can turn off (General is always on).
fn optional() -> impl Iterator<Item = Capability> {
    Capability::ALL
        .into_iter()
        .filter(|c| *c != Capability::General)
}

impl Chrome {
    /// Opens the first-run question with `current` checked.
    pub fn ask_profile(&mut self, current: Capabilities) {
        self.profile_pick = current.with(Capability::General);
        self.modal = None;
        self.open_modal(Modal::Profile);
    }

    /// The start choice the first-run sheet shows selected (the current `general.on_start`).
    pub fn set_profile_start(&mut self, o: OnStart) {
        self.profile_start = o;
    }

    /// The question's keyboard stops: the check boxes that can change, the start choices,
    /// then Skip and Continue.
    pub(crate) fn profile_stops(&self) -> Vec<Target> {
        optional()
            .map(Target::ProfileCheck)
            .chain(STARTS.map(Target::ProfileStart))
            .chain([Target::ProfileSkip, Target::ProfileDone])
            .collect()
    }

    /// ↑ or ↓ on a start choice: the one before or after is chosen (and focused).
    pub(crate) fn profile_start_step(&mut self, by: i32) {
        let at = STARTS
            .iter()
            .position(|o| *o == self.profile_start)
            .unwrap_or(0) as i32;
        let to = (at + by).clamp(0, STARTS.len() as i32 - 1) as usize;
        self.profile_start = STARTS[to];
        self.modal_focus = Some(Target::ProfileStart(STARTS[to]));
        self.reveal_focus = true;
    }

    /// Skip: closes the question, keeping every default.
    pub(crate) fn profile_skip(&mut self) -> Response {
        self.close_modal();
        Response::used()
    }

    /// A check box of the question flipped (General stays on).
    pub(crate) fn profile_toggle(&mut self, c: Capability) {
        if c == Capability::General {
            return;
        }
        self.profile_pick = if self.profile_pick.contains(c) {
            self.profile_pick.without(c)
        } else {
            self.profile_pick.with(c)
        };
    }

    /// Continue: the choice is stored (and applies at once) and the question closes.
    pub(crate) fn profile_accept(&mut self, st: &ViewerState) -> Response {
        let list: Vec<ProfileCapability> = self
            .profile_pick
            .iter()
            .filter_map(|c| ProfileCapability::from_name(c.id()))
            .collect();
        let start = self.profile_start;
        st.store.update(Origin::SettingsWindow, |s| {
            s.profile.capabilities = list;
            s.general.on_start = start;
        });
        self.close_modal();
        Response::used()
    }
}

/// Draws the question, centred.
pub(crate) fn draw(ch: &Chrome, cx: &mut Ctx) {
    let w = cx.u(480.0).min(cx.w - 32.0);
    let inner = w - 48.0;
    let ts = cx.style(metrics::FONT_TITLE).weight(600.0);
    let tb = cx.layout(&t!("profile-question"), ts, Some(inner));
    let ss = cx.style(metrics::FONT_LABEL);
    let ib = cx.layout(&t!("profile-intro"), ss, Some(inner));
    let ls = cx.style(metrics::FONT_BODY);
    let box_s = cx.u(18.0);
    let lx = 8.0 + box_s + 12.0;
    let text_w = w - 32.0 - lx - 8.0;
    // Each choice: its name, and under it what it is for (left out when the window is too short
    // for them; screen readers still get them). Shorter still, they go in two columns.
    let col_text_w = ((w - 32.0) * 0.5 - lx - 8.0).max(1.0);
    let choices_fn = |cx: &mut Ctx, about_too: bool, cols: usize| -> Vec<_> {
        let lw = if cols > 1 { col_text_w } else { text_w };
        Capability::ALL
            .into_iter()
            .map(|c| {
                let lb = cx.layout(&label(c), ls, Some(lw));
                let ab = about_too.then(|| cx.layout(&about(c), ss, Some(text_w)));
                let ah = ab.as_ref().map_or(0.0, |b| 2.0 + b.height);
                let h = (8.0 + lb.height + ah + 8.0).max(cx.u(36.0));
                (c, lb, ab, h)
            })
            .collect()
    };
    let mut cols = 1usize;
    let mut choices = choices_fn(cx, true, cols);
    let bs = cx.style(metrics::FONT_LABEL).weight(550.0);
    let db = cx.layout(&t!("profile-done"), bs, None);
    let bh = cx.u(34.0);
    let bw = (db.width + 40.0).min(inner);
    let nb = cx.layout(&t!("profile-later"), ss, Some(inner));
    let skb = cx.layout(&t!("profile-skip"), bs, None);
    let sw = (skb.width + 32.0).min(inner);
    // What a start without a file shows: a heading and a radio button a row.
    let hs = cx.style(metrics::FONT_LABEL).weight(600.0);
    let shb = cx.layout(&t!("profile-on-start"), hs, Some(inner));
    let start_row = cx.u(30.0);
    let starts: Vec<_> = STARTS
        .iter()
        .map(|o| (*o, cx.layout(&start_label(*o), ls, Some(text_w))))
        .collect();
    let start_rows: f32 = starts
        .iter()
        .map(|(_, b)| b.height.max(start_row - 8.0) + 8.0)
        .sum();
    let start_h = 16.0 + shb.height + 6.0 + start_rows;
    let height = |choices: &[(Capability, _, _, f32)], cols: usize| {
        let rows: f32 = choices
            .chunks(cols)
            .map(|c| c.iter().map(|c| c.3).fold(0.0, f32::max))
            .sum();
        24.0 + tb.height
            + 8.0
            + ib.height
            + 16.0
            + rows
            + start_h
            + 16.0
            + nb.height
            + 16.0
            + bh
            + 20.0
    };
    let mut h = height(&choices, cols);
    if h > cx.h - 16.0 {
        choices = choices_fn(cx, false, cols);
        h = height(&choices, cols);
    }
    if h > cx.h - 16.0 {
        cols = 2;
        choices = choices_fn(cx, false, cols);
        h = height(&choices, cols);
    }
    let cell_w = (w - 32.0) / cols as f32;
    let lw = if cols > 1 { col_text_w } else { text_w };
    let r = Rect::new(((cx.w - w) * 0.5).round(), ((cx.h - h) * 0.5).round(), w, h);
    cx.sheet(r);
    let tr = cx.m_in(Rect::new(r.x + 24.0, r.y + 24.0, inner, tb.height), r);
    cx.block_top(&tb, tr, HAlign::Start, cx.pal.text);
    let iy = r.y + 24.0 + tb.height + 8.0;
    let ir = cx.m_in(Rect::new(r.x + 24.0, iy, inner, ib.height), r);
    cx.block_top(&ib, ir, HAlign::Start, cx.pal.text_muted);

    let mut y = iy + ib.height + 16.0;
    let n_choices = choices.len();
    for (i, (c, lb, ab, row)) in choices.into_iter().enumerate() {
        let locked = c == Capability::General;
        let on = ch.profile_pick.contains(c);
        let t = Target::ProfileCheck(c);
        let col = (i % cols) as f32;
        let rr = cx.m_in(
            Rect::new(r.x + 16.0 + col * cell_w, y, cell_w, row),
            Rect::new(r.x + 16.0, y, w - 32.0, row),
        );
        if !locked && cx.is_hover(&t) {
            cx.dl.rect(rr, metrics::RADIUS_SM, cx.pal.surface_hover);
        }
        // The box lines up with the name, the first line.
        let br = cx
            .m_in(Rect::new(rr.x + 8.0, rr.y + 8.0, box_s, lb.height), rr)
            .centered(box_s, box_s);
        let ink = if locked {
            cx.pal.text_faint
        } else {
            cx.pal.accent_fill
        };
        if on {
            cx.dl.rect(br, metrics::RADIUS_SM, ink);
            draw_icon(
                cx.dl,
                Icon::Check,
                br.inset(2.0),
                cx.pal.accent_text,
                2.0,
                false,
            );
        } else {
            cx.dl.rect_border(
                br,
                metrics::RADIUS_SM,
                cx.pal.surface_raised,
                cx.pal.text_muted,
                1.5,
            );
        }
        let lr = cx.m_in(Rect::new(rr.x + lx, rr.y + 8.0, lw, lb.height), rr);
        let color = if locked {
            cx.pal.text_muted
        } else {
            cx.pal.text
        };
        cx.block_top(&lb, lr, HAlign::Start, color);
        let ah = ab.as_ref().map_or(0.0, |b| b.height);
        let ar = cx.m_in(
            Rect::new(rr.x + lx, rr.y + 8.0 + lb.height + 2.0, lw, ah),
            rr,
        );
        if let Some(ab) = &ab {
            cx.block_top(ab, ar, HAlign::Start, cx.pal.text_muted);
        }
        cx.ring_if(&t, rr, metrics::RADIUS_SM);
        cx.hit(rr, t);
        if (i + 1) % cols == 0 || i + 1 == n_choices {
            y += row;
        }
    }

    // The start choice.
    y += 16.0;
    let hr = cx.m_in(Rect::new(r.x + 24.0, y, inner, shb.height), r);
    cx.block_top(&shb, hr, HAlign::Start, cx.pal.text);
    y += shb.height + 6.0;
    for (o, lb) in &starts {
        let row = lb.height.max(start_row - 8.0) + 8.0;
        let t = Target::ProfileStart(*o);
        let rr = Rect::new(r.x + 16.0, y, w - 32.0, row);
        if cx.is_hover(&t) {
            cx.dl.rect(rr, metrics::RADIUS_SM, cx.pal.surface_hover);
        }
        let dot = cx
            .m_in(Rect::new(rr.x + 8.0, rr.y + 4.0, box_s, lb.height), rr)
            .centered(box_s, box_s);
        let on = ch.profile_start == *o;
        cx.dl.rect_border(
            dot,
            box_s * 0.5,
            cx.pal.surface_raised,
            if on { cx.pal.accent } else { cx.pal.text_muted },
            if on { 2.0 } else { 1.5 },
        );
        if on {
            let d = box_s * 0.5;
            cx.dl.rect(dot.centered(d, d), d * 0.5, cx.pal.accent);
        }
        let lr = cx.m_in(Rect::new(rr.x + lx, rr.y + 4.0, text_w, lb.height), rr);
        cx.block_top(lb, lr, HAlign::Start, cx.pal.text);
        cx.ring_if(&t, rr, metrics::RADIUS_SM);
        cx.hit(rr, t);
        y += row;
    }

    let ny = y + 16.0;
    let nr = cx.m_in(Rect::new(r.x + 24.0, ny, inner, nb.height), r);
    cx.block_top(&nb, nr, HAlign::Start, cx.pal.text_muted);

    let by = r.bottom() - 20.0 - bh;
    let dr = cx.m_in(Rect::new(r.right() - 24.0 - bw, by, bw, bh), r);
    let t = Target::ProfileDone;
    let fill = if cx.is_hover(&t) {
        cx.pal.accent_fill.lerp(cx.pal.accent_text, 0.12)
    } else {
        cx.pal.accent_fill
    };
    let radius = metrics::RADIUS_SM + 2.0;
    cx.dl.rect(dr, radius, fill);
    if cx.is_focused(&t) {
        cx.button_ring(dr, radius);
    }
    cx.block_in(&db, dr, HAlign::Center, cx.pal.accent_text);
    cx.hit(dr, t);

    // Skip, before Continue.
    let kr = cx.m_in(Rect::new(r.right() - 24.0 - bw - 10.0 - sw, by, sw, bh), r);
    let t = Target::ProfileSkip;
    let fill = if cx.is_hover(&t) {
        cx.pal.surface_hover
    } else {
        cx.pal.surface_raised
    };
    cx.dl.rect_border(kr, radius, fill, cx.pal.separator, 1.0);
    if cx.is_focused(&t) {
        cx.button_ring(kr, radius);
    }
    cx.block_in(&skb, kr, HAlign::Center, cx.pal.text);
    cx.hit(kr, t);
}
