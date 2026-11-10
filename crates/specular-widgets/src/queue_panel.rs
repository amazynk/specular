//! The play queue panel: what plays after the current item, in order, with the one playing
//! marked. Rows are dragged to reorder them, double-clicked (or Enter) to play, removed with
//! their × (or Delete; the files stay where they are). Repeat, Shuffle, Save as playlist and
//! Clear sit above the list. Network items from a playlist are listed but not played.
//!
//! The app localises the content ([`QueueView`]); choices come back as
//! [`Command::Queue`]. The transport's repeat and shuffle buttons show [`PlayOrder`].

use std::sync::Arc;

use specular_gfx::{Icon, Rect, TextBlock};
use specular_i18n::t;
use specular_input::{Key, KeyCombo, NamedKey};
use specular_settings::PlayRepeat;
use specular_theme::metrics;

use crate::ctx::{Ctx, HAlign, Target};
use crate::{Chrome, Command, Modal, Response};

/// What the queue panel shows, localised by the app.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QueueView {
    pub rows: Vec<QueueRow>,
    /// "12 items · 48 min".
    pub summary: String,
}

/// One item of the queue.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QueueRow {
    /// Its title (from the playlist or tags), else its file name.
    pub title: String,
    /// Its folder or artist; for a network item, why it does not play.
    pub detail: String,
    /// "3:25", or empty while not known.
    pub duration: String,
    /// It is the item playing.
    pub current: bool,
    /// It plays (a file); a network item does not.
    pub playable: bool,
}

/// Repeat and shuffle as set (the transport's buttons and the panel's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayOrder {
    pub repeat: PlayRepeat,
    pub shuffle: bool,
}

impl Default for PlayOrder {
    fn default() -> Self {
        PlayOrder {
            repeat: PlayRepeat::Off,
            shuffle: false,
        }
    }
}

/// A choice made in the queue panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueueCmd {
    /// Play item `i` now.
    Play(usize),
    /// Take item `i` out of the queue.
    Remove(usize),
    /// Item `from` goes to place `to`.
    Move {
        from: usize,
        to: usize,
    },
    /// Empty the queue (the app asks first).
    Clear,
    /// Save the queue as a playlist (the app asks where).
    Save,
    /// The next repeat mode.
    Repeat,
    Shuffle,
}

/// The panel's controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum QueueTarget {
    Row(usize),
    Remove(usize),
    Repeat,
    Shuffle,
    Save,
    Clear,
}

/// The panel's state in the chrome.
#[derive(Debug, Default)]
pub(crate) struct QueueState {
    pub view: QueueView,
    pub order: Option<PlayOrder>,
    pub scroll: f32,
    /// A row pressed: its index, where the press was and where the pointer is (logical y),
    /// and whether it has moved far enough to be a drag.
    drag: Option<Drag>,
    /// The last click on a row and when (for a double click).
    click: Option<(usize, f64)>,
    /// The list as last drawn: its rectangle and the row height.
    list: Option<(Rect, f32)>,
}

#[derive(Clone, Copy, Debug)]
struct Drag {
    from: usize,
    y0: f32,
    y: f32,
    active: bool,
}

/// Movement past which a press on a row is a drag.
const DRAG_SLOP: f32 = 5.0;
/// Two clicks on a row within this time play it (seconds).
const DOUBLE_CLICK: f64 = 0.45;

impl Chrome {
    /// What the queue panel lists (the app sets it when the queue changes).
    pub fn set_queue(&mut self, v: QueueView) {
        let n = v.rows.len();
        self.queue.view = v;
        if let Some(Target::Queue(QueueTarget::Row(i) | QueueTarget::Remove(i))) = self.modal_focus
            && i >= n
        {
            self.modal_focus = n
                .checked_sub(1)
                .map(|l| Target::Queue(QueueTarget::Row(l)))
                .or(Some(Target::ModalClose));
        }
        if self.queue.drag.is_some_and(|d| d.from >= n) {
            self.queue.drag = None;
        }
    }

    /// Repeat and shuffle as set (`None`: no player on screen).
    pub fn set_play_order(&mut self, o: Option<PlayOrder>) {
        self.queue.order = o;
    }

    pub fn play_order(&self) -> Option<PlayOrder> {
        self.queue.order
    }

    /// Opens the queue panel.
    pub fn show_queue(&mut self) {
        self.open_modal(Modal::Queue);
    }

    pub fn queue_open(&self) -> bool {
        self.modal == Some(Modal::Queue)
    }

    pub(crate) fn queue_scroll_mut(&mut self) -> &mut f32 {
        &mut self.queue.scroll
    }

    pub(crate) fn queue_reset(&mut self) {
        self.queue.scroll = 0.0;
        self.queue.drag = None;
        self.queue.click = None;
        self.modal_focus = None;
    }

    /// The panel's buttons in Tab order: Repeat, Shuffle, Save, Clear, each row, Close.
    pub(crate) fn queue_stops(&self) -> Vec<Target> {
        let mut v = vec![
            Target::Queue(QueueTarget::Repeat),
            Target::Queue(QueueTarget::Shuffle),
        ];
        if !self.queue.view.rows.is_empty() {
            v.push(Target::Queue(QueueTarget::Save));
            v.push(Target::Queue(QueueTarget::Clear));
        }
        v.extend((0..self.queue.view.rows.len()).map(|i| Target::Queue(QueueTarget::Row(i))));
        v.push(Target::ModalClose);
        v
    }

    /// A click (or Enter, Space) on a control of the panel. A row's click is handled on
    /// release ([`Chrome::queue_release`]); here, a row is the keyboard's: it plays.
    pub(crate) fn queue_click(&mut self, t: QueueTarget) -> Response {
        let cmd = match t {
            QueueTarget::Row(i) => QueueCmd::Play(i),
            QueueTarget::Remove(i) => QueueCmd::Remove(i),
            QueueTarget::Repeat => QueueCmd::Repeat,
            QueueTarget::Shuffle => QueueCmd::Shuffle,
            QueueTarget::Save => QueueCmd::Save,
            QueueTarget::Clear => QueueCmd::Clear,
        };
        Response::cmd(Command::Queue(cmd))
    }

    /// A press on a row starts what may be a drag.
    pub(crate) fn queue_press(&mut self, h: Option<&Target>, p: (f32, f32)) {
        self.queue.drag = match (self.modal, h) {
            (Some(Modal::Queue), Some(Target::Queue(QueueTarget::Row(i)))) => Some(Drag {
                from: *i,
                y0: p.1,
                y: p.1,
                active: false,
            }),
            _ => None,
        };
    }

    /// The pointer moved: a pressed row follows it once past the slop. Whether it was ours.
    pub(crate) fn queue_drag_move(&mut self, p: (f32, f32)) -> bool {
        let Some(d) = self.queue.drag.as_mut() else {
            return false;
        };
        d.y = p.1;
        if !d.active && (d.y - d.y0).abs() > DRAG_SLOP {
            d.active = true;
        }
        d.active
    }

    /// The button let go over the panel: a drag drops its row; a click on a row focuses it,
    /// a second click on it soon after plays it. `None`: not ours.
    pub(crate) fn queue_release(&mut self, h: Option<&Target>, now: f64) -> Option<Response> {
        let d = self.queue.drag.take()?;
        if d.active {
            let to = self.queue_drop_index(d.y).unwrap_or(d.from);
            self.queue.click = None;
            if to == d.from {
                return Some(Response::used());
            }
            self.modal_focus = Some(Target::Queue(QueueTarget::Row(to)));
            return Some(Response::cmd(Command::Queue(QueueCmd::Move {
                from: d.from,
                to,
            })));
        }
        let i = match h {
            Some(Target::Queue(QueueTarget::Row(i))) if *i == d.from => *i,
            _ => return None,
        };
        self.modal_focus = Some(Target::Queue(QueueTarget::Row(i)));
        match self.queue.click.take() {
            Some((j, t)) if j == i && now - t <= DOUBLE_CLICK => {
                Some(Response::cmd(Command::Queue(QueueCmd::Play(i))))
            }
            _ => {
                self.queue.click = Some((i, now));
                Some(Response::used())
            }
        }
    }

    /// Where a row dropped at `y` goes: the row under it (the first or last past the ends).
    fn queue_drop_index(&self, y: f32) -> Option<usize> {
        let (list, row_h) = self.queue.list?;
        let n = self.queue.view.rows.len();
        if n == 0 {
            return None;
        }
        let k = ((y - list.y + self.queue.scroll) / row_h).floor();
        Some((k.max(0.0) as usize).min(n - 1))
    }

    /// The panel's own keys while a row has the keyboard: Delete removes it, the arrows move
    /// between rows, Alt with them moves the row. Enter (play) is the focus's.
    pub(crate) fn queue_key(&mut self, combo: &KeyCombo, repeat: bool) -> Option<Response> {
        let Some(Target::Queue(QueueTarget::Row(i))) = self.modal_focus else {
            return None;
        };
        let n = self.queue.view.rows.len();
        let Key::Named(k) = combo.key else {
            return None;
        };
        let m = combo.mods;
        let plain = !m.any();
        let alt_only = m.alt && !m.ctrl && !m.shift && !m.meta;
        let focus = |ch: &mut Chrome, j: usize| {
            ch.modal_focus = Some(Target::Queue(QueueTarget::Row(j)));
            ch.reveal_focus = true;
            Some(Response::used())
        };
        match k {
            NamedKey::Delete | NamedKey::Backspace if plain => {
                if repeat {
                    return Some(Response::used());
                }
                Some(Response::cmd(Command::Queue(QueueCmd::Remove(i))))
            }
            NamedKey::ArrowUp | NamedKey::ArrowDown if alt_only => {
                let to = if k == NamedKey::ArrowUp {
                    i.checked_sub(1)?
                } else {
                    (i + 1 < n).then_some(i + 1)?
                };
                self.modal_focus = Some(Target::Queue(QueueTarget::Row(to)));
                self.reveal_focus = true;
                Some(Response::cmd(Command::Queue(QueueCmd::Move {
                    from: i,
                    to,
                })))
            }
            NamedKey::ArrowUp if plain => focus(self, i.saturating_sub(1)),
            NamedKey::ArrowDown if plain => focus(self, (i + 1).min(n.saturating_sub(1))),
            NamedKey::Home if plain => focus(self, 0),
            NamedKey::End if plain => focus(self, n.saturating_sub(1)),
            _ => None,
        }
    }
}

/// The label of the repeat control for `r`.
pub fn repeat_label(r: PlayRepeat) -> String {
    match r {
        PlayRepeat::Off => t!("queue-repeat-off"),
        PlayRepeat::All => t!("queue-repeat-all"),
        PlayRepeat::One => t!("queue-repeat-one"),
    }
}

/// The repeat control's icon for `r`.
pub(crate) fn repeat_icon(r: PlayRepeat) -> Icon {
    match r {
        PlayRepeat::One => Icon::RepeatOne,
        _ => Icon::Repeat,
    }
}

/// The label of the shuffle control.
pub fn shuffle_label(on: bool) -> String {
    if on {
        t!("queue-shuffle-on")
    } else {
        t!("queue-shuffle-off")
    }
}

/// A row laid out.
struct Row {
    title: Arc<TextBlock>,
    detail: Arc<TextBlock>,
    duration: Option<Arc<TextBlock>>,
}

/// A text button of the panel's bar: its target, icon, label and whether it is on.
struct Button {
    t: QueueTarget,
    icon: Icon,
    label: Arc<TextBlock>,
    on: bool,
}

/// The "Play queue" panel.
pub(crate) fn draw(ch: &mut Chrome, cx: &mut Ctx) {
    let w = (cx.w - 48.0).min(cx.u(580.0));
    let inner_w = w - 48.0;
    let title_s = cx.style(metrics::FONT_TITLE).weight(650.0);
    let body_s = cx.style(metrics::FONT_BODY);
    let small_s = cx.style(metrics::FONT_SMALL);
    let button_s = cx.style(metrics::FONT_LABEL).weight(550.0);
    let bh = cx.u(30.0);
    let icon = cx.u(16.0);
    let row_h = cx.u(48.0);
    let remove_w = cx.u(28.0);
    let v = ch.queue.view.clone();
    let order = ch.queue.order.unwrap_or_default();

    let title = cx.layout(&t!("queue-title"), title_s, Some(inner_w - 40.0));
    let summary = (!v.summary.is_empty()).then(|| cx.layout(&v.summary, small_s, Some(inner_w)));
    let mut buttons = vec![
        Button {
            t: QueueTarget::Repeat,
            icon: repeat_icon(order.repeat),
            label: cx.layout(&repeat_label(order.repeat), button_s, None),
            on: order.repeat != PlayRepeat::Off,
        },
        Button {
            t: QueueTarget::Shuffle,
            icon: Icon::Shuffle,
            label: cx.layout(&t!("queue-shuffle"), button_s, None),
            on: order.shuffle,
        },
    ];
    if !v.rows.is_empty() {
        buttons.push(Button {
            t: QueueTarget::Save,
            icon: Icon::Download,
            label: cx.layout(&t!("queue-save"), button_s, None),
            on: false,
        });
        buttons.push(Button {
            t: QueueTarget::Clear,
            icon: Icon::Trash,
            label: cx.layout(&t!("queue-clear"), button_s, None),
            on: false,
        });
    }
    let button_w = |b: &Button| (12.0 + icon + 6.0 + b.label.width + 14.0).ceil();
    // The buttons in lines that fit.
    let mut lines: Vec<Vec<usize>> = vec![Vec::new()];
    let mut x = 0.0;
    for (k, b) in buttons.iter().enumerate() {
        let bw = button_w(b);
        if x > 0.0 && x + bw > inner_w {
            lines.push(Vec::new());
            x = 0.0;
        }
        lines.last_mut().unwrap().push(k);
        x += bw + 8.0;
    }

    let rows: Vec<Row> = v
        .rows
        .iter()
        .map(|r| {
            let dur =
                (!r.duration.is_empty()).then(|| cx.layout(&r.duration, small_s.tabular(), None));
            let dur_w = dur.as_ref().map_or(0.0, |d| d.width + 12.0);
            let text_w = (inner_w - icon - 10.0 - dur_w - remove_w - 8.0).max(40.0);
            let title = cx.ellipsize(&r.title, body_s, text_w, false);
            let detail = cx.ellipsize(&r.detail, small_s, text_w, true);
            Row {
                title: cx.layout(&title, body_s, None),
                detail: cx.layout(&detail, small_s, None),
                duration: dur,
            }
        })
        .collect();
    let empty = rows
        .is_empty()
        .then(|| cx.layout(&t!("queue-empty"), small_s, Some(inner_w)));
    let content = match &empty {
        Some(e) => e.height,
        None => row_h * rows.len() as f32,
    };
    let head = title.height + summary.as_ref().map_or(0.0, |s| 4.0 + s.height) + 16.0;
    let bar_h = lines.len() as f32 * (bh + 8.0) + 8.0;
    let h = (24.0 + head + bar_h + content + 24.0).min(cx.h - 32.0);
    let r = Rect::new(((cx.w - w) * 0.5).round(), ((cx.h - h) * 0.5).round(), w, h);
    cx.sheet(r);
    let close = cx.m_in(Rect::new(r.right() - 46.0, r.y + 14.0, 32.0, 32.0), r);
    cx.close_button(close);
    let tr = cx.m_in(
        Rect::new(r.x + 24.0, r.y + 24.0, inner_w - 40.0, title.height),
        r,
    );
    cx.block_top(&title, tr, HAlign::Start, cx.pal.text);
    if let Some(s) = &summary {
        let sr = cx.m_in(
            Rect::new(
                r.x + 24.0,
                r.y + 24.0 + title.height + 4.0,
                s.width,
                s.height,
            ),
            r,
        );
        cx.block_top(s, sr, HAlign::Start, cx.pal.text_muted);
    }

    // The bar: Repeat, Shuffle, Save, Clear.
    let mut y = r.y + 24.0 + head;
    for line in &lines {
        let mut x = r.x + 24.0;
        for &k in line {
            let b = &buttons[k];
            let bw = button_w(b);
            let br = cx.m_in(Rect::new(x, y, bw, bh), r);
            let t = Target::Queue(b.t);
            let fill = if b.on {
                cx.pal.accent.with_alpha(0.22)
            } else if cx.is_pressed(&t) {
                cx.pal.surface_pressed
            } else if cx.is_hover(&t) {
                cx.pal.surface_hover
            } else {
                cx.pal.surface_raised
            };
            let edge = if b.on {
                cx.pal.accent
            } else {
                cx.pal.separator
            };
            cx.dl
                .rect_border(br, metrics::RADIUS_SM + 2.0, fill, edge, 1.0);
            let ir = cx.m_in(Rect::new(x + 12.0, y + (bh - icon) * 0.5, icon, icon), r);
            let tint = if b.on { cx.pal.accent } else { cx.pal.text };
            specular_gfx::draw_icon(cx.dl, b.icon, ir, tint, 1.6, false);
            let lr = cx.m_in(Rect::new(x + 12.0 + icon + 6.0, y, b.label.width, bh), r);
            cx.block_in(&b.label, lr, HAlign::Start, cx.pal.text);
            cx.ring_if(&t, br, metrics::RADIUS_SM + 2.0);
            cx.hit(br, t);
            x += bw + 8.0;
        }
        y += bh + 8.0;
    }

    let body = Rect::new(r.x + 24.0, y, inner_w, (r.bottom() - 24.0 - y).max(0.0));
    let max = (content - body.h).max(0.0);
    if std::mem::take(&mut ch.reveal_focus)
        && let Some(Target::Queue(QueueTarget::Row(i))) = ch.modal_focus
    {
        let top = row_h * i as f32;
        let s = &mut ch.queue.scroll;
        *s = s.min(top).max(top + row_h - body.h);
    }
    ch.queue.scroll = ch.queue.scroll.clamp(0.0, max);
    ch.queue.list = Some((body, row_h));
    cx.dl.push_clip(body);
    if let Some(e) = &empty {
        let er = cx.m_in(Rect::new(body.x, body.y, e.width, e.height), body);
        cx.block_top(e, er, HAlign::Start, cx.pal.text_muted);
    }
    let drag = ch.queue.drag.filter(|d| d.active);
    let drop = drag.and_then(|d| ch.queue_drop_index(d.y).map(|to| (d.from, to)));
    let visible = |rr: Rect| rr.bottom() > body.y && rr.y < body.bottom();
    let mut rings = Vec::new();
    for (i, (row, data)) in rows.iter().zip(&v.rows).enumerate() {
        let ry = body.y - ch.queue.scroll + row_h * i as f32;
        let rr = Rect::new(body.x, ry, inner_w, row_h);
        if !visible(rr) {
            continue;
        }
        let t = Target::Queue(QueueTarget::Row(i));
        let dragged = drag.is_some_and(|d| d.from == i);
        let bg = if data.current {
            Some(cx.pal.accent.with_alpha(0.16))
        } else if cx.is_hover(&t) && drag.is_none() {
            Some(cx.pal.surface_hover)
        } else {
            None
        };
        if let Some(bg) = bg {
            cx.dl.rect(cx.m_in(rr, body), metrics::RADIUS_SM, bg);
        }
        let alpha = if dragged { 0.4 } else { 1.0 };
        cx.dl.push_opacity(alpha);
        // The item playing: a play mark; the rest their number.
        let mark = cx.m_in(
            Rect::new(rr.x + 2.0, ry + (row_h - icon) * 0.5, icon, icon),
            body,
        );
        if data.current {
            specular_gfx::draw_icon(cx.dl, Icon::Play, mark, cx.pal.accent, 1.8, cx.rtl);
        } else {
            let num = cx.layout(&(i + 1).to_string(), small_s.tabular(), None);
            cx.block_in(&num, mark, HAlign::Center, cx.pal.text_faint);
        }
        let tx = rr.x + icon + 10.0;
        let ty = ry + (row_h - row.title.height - 2.0 - row.detail.height) * 0.5;
        let title_c = if data.playable {
            cx.pal.text
        } else {
            cx.pal.text_muted
        };
        let tr = cx.m_in(Rect::new(tx, ty, row.title.width, row.title.height), body);
        cx.block_top(&row.title, tr, HAlign::Start, title_c);
        let dr = cx.m_in(
            Rect::new(
                tx,
                ty + row.title.height + 2.0,
                row.detail.width,
                row.detail.height,
            ),
            body,
        );
        let detail_c = if data.playable {
            cx.pal.text_muted
        } else {
            cx.pal.warning
        };
        cx.block_top(&row.detail, dr, HAlign::Start, detail_c);
        let xr = Rect::new(
            rr.right() - remove_w,
            ry + (row_h - remove_w) * 0.5,
            remove_w,
            remove_w,
        );
        if let Some(d) = &row.duration {
            let dr = cx.m_in(Rect::new(xr.x - 8.0 - d.width, ry, d.width, row_h), body);
            cx.block_in(d, dr, HAlign::Start, cx.pal.text_muted);
        }
        cx.dl.pop_opacity();
        if cx.is_focused(&t) {
            rings.push(cx.m_in(rr, body));
        }
        // The part of the row inside the list takes the pointer (not the bar above it).
        let (y0, y1) = (rr.y.max(body.y), rr.bottom().min(body.bottom()));
        cx.hit(cx.m_in(Rect::new(rr.x, y0, rr.w, y1 - y0), body), t.clone());
        // Remove: shown on the row under the pointer, the focused row and the one playing.
        let rt = Target::Queue(QueueTarget::Remove(i));
        let whole = xr.y >= body.y && xr.bottom() <= body.bottom();
        if whole
            && drag.is_none()
            && (cx.is_hover(&t) || cx.is_hover(&rt) || cx.is_focused(&t) || data.current)
        {
            let xr = cx.m_in(xr, body);
            cx.icon_button(xr, Icon::Close, rt, cx.pal.text_muted, false, false);
        }
    }
    // Where a dragged row lands: a line above or below the row it is over.
    if let Some((from, to)) = drop
        && from != to
    {
        let below = to > from;
        let ly = body.y - ch.queue.scroll + row_h * (to + usize::from(below)) as f32;
        let line = cx.m_in(Rect::new(body.x, ly - 1.0, inner_w, 2.0), body);
        cx.dl.rect(line, 1.0, cx.pal.accent);
    }
    cx.dl.pop_clip();
    if !rings.is_empty() {
        cx.dl
            .push_clip(Rect::new(body.x - 4.0, body.y, body.w + 8.0, body.h));
        for rr in rings {
            cx.button_ring(rr, metrics::RADIUS_SM);
        }
        cx.dl.pop_clip();
    }
    crate::scrollbar::draw(
        ch,
        cx,
        crate::scrollbar::Area::Queue,
        body,
        Rect::new(r.x, body.y, r.w, body.h),
        max,
    );
}

/// The tooltip of the transport's repeat and shuffle buttons (their state) and of the
/// panel's controls, with the shortcut.
pub(crate) fn tooltip(ch: &Chrome, t: &Target, st: &crate::ViewerState) -> Option<String> {
    use specular_input::Action;
    let order = ch.queue.order.unwrap_or_default();
    let (label, action) = match t {
        Target::Transport(Action::CycleRepeat) | Target::Queue(QueueTarget::Repeat) => {
            (repeat_label(order.repeat), Some(Action::CycleRepeat))
        }
        Target::Transport(Action::ToggleShuffle) | Target::Queue(QueueTarget::Shuffle) => {
            (shuffle_label(order.shuffle), Some(Action::ToggleShuffle))
        }
        Target::Queue(QueueTarget::Remove(_)) => (t!("queue-remove"), None),
        Target::Queue(QueueTarget::Save) => (t!("queue-save-tip"), Some(Action::SaveQueue)),
        Target::Queue(QueueTarget::Clear) => (t!("queue-clear-tip"), None),
        Target::Queue(QueueTarget::Row(_)) => return None,
        _ => return t.tooltip(st),
    };
    if let Some(why) = t.disabled(st) {
        let reason = specular_i18n::I18n::global().tr(why.message_id());
        return Some(t!("tooltip-unavailable", label = label, reason = reason));
    }
    let key = action.and_then(|a| st.keymap.primary_key(a));
    Some(match key {
        Some(k) => t!(
            "tooltip-with-shortcut",
            label = label,
            shortcut = k.to_string()
        ),
        None => label,
    })
}

/// Screen-reader facts of a panel control: a row reads its title, then whether it is the
/// one playing, what it is and its length.
pub(crate) fn describe(ch: &Chrome, q: QueueTarget, n: &mut accesskit::Node) -> Option<()> {
    use accesskit::{Role, Toggled};
    let order = ch.queue.order.unwrap_or_default();
    match q {
        QueueTarget::Row(i) => {
            let r = ch.queue.view.rows.get(i)?;
            n.set_role(Role::ListItem);
            n.set_label(r.title.as_str());
            let mut d = r.detail.clone();
            if !r.duration.is_empty() {
                d = t!("net-join", first = d, second = r.duration.as_str());
            }
            if r.current {
                d = t!("net-join", first = t!("queue-playing"), second = d);
            }
            n.set_description(d);
            n.set_selected(r.current);
        }
        QueueTarget::Remove(i) => {
            let r = ch.queue.view.rows.get(i)?;
            n.set_label(t!("queue-remove"));
            n.set_description(r.title.as_str());
        }
        QueueTarget::Repeat => n.set_label(repeat_label(order.repeat)),
        QueueTarget::Shuffle => {
            n.set_label(t!("queue-shuffle"));
            n.set_toggled(if order.shuffle {
                Toggled::True
            } else {
                Toggled::False
            });
        }
        QueueTarget::Save => n.set_label(t!("queue-save")),
        QueueTarget::Clear => n.set_label(t!("queue-clear")),
    }
    Some(())
}
