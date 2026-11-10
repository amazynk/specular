//! A read-only block of rows the application puts under one setting of the settings window
//! (Settings → Performance → "How Specular runs on this computer", under "Where images are
//! processed"): a heading, one row per item (name, value, plain-language lines, a detail line
//! with the numbers, an optional button), a footer line and one action with a confirm step.
//!
//! The widgets know nothing of what the rows mean: the application builds the text
//! ([`Chrome::set_setting_panel`]) and gets [`Command::SettingPanel`] back for the buttons.

use specular_gfx::{Icon, Rect, draw_icon};
use specular_i18n::fold_for_search;
use specular_theme::metrics;

use crate::Chrome;
use crate::ctx::{Ctx, HAlign, Target};
use crate::settings::SettingsTarget;

/// One row of a [`SettingPanel`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PanelRow {
    /// Sent back with [`PanelEvent::Row`] when its button is pressed.
    pub id: String,
    /// What the row is about ("Opening JPEG photos").
    pub label: String,
    /// The outcome, shown at the end of the row ("Graphics processor").
    pub value: String,
    /// Why, in plain language (one paragraph each).
    pub lines: Vec<String>,
    /// The numbers behind it, smaller and fainter.
    pub detail: String,
    /// Value and first line in the accent colour (blocked, overridden).
    pub attention: bool,
    /// A button at the end of the row ("Try again").
    pub button: Option<String>,
    /// A checkbox before the label: a choice the application keeps
    /// ([`PanelEvent::Check`] when pressed, on the box or the label).
    pub check: Option<CheckState>,
    /// A disclosure arrow before the label: whether the rows under it are shown
    /// ([`PanelEvent::Expand`] when pressed). The application leaves out the rows of a
    /// collapsed row.
    pub expand: Option<bool>,
    /// How far the row is indented (0, 1, 2): the rows under an expandable one.
    pub indent: u8,
}

/// A row's checkbox.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CheckState {
    #[default]
    Off,
    On,
    /// Some of the rows under it are on (a group).
    Mixed,
}

/// The panel's one action, with a confirm step ("Measure again" → "Forget measurements").
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PanelAction {
    pub label: String,
    /// The question shown while confirming.
    pub question: String,
    pub confirm: String,
    pub cancel: String,
}

/// A block of rows under a setting (see the module docs).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SettingPanel {
    pub title: String,
    /// Under the title (may be empty).
    pub intro: String,
    pub rows: Vec<PanelRow>,
    /// Under the rows, small (e.g. which machine the measurements are for).
    pub footer: String,
    pub action: Option<PanelAction>,
    /// Extra words settings search finds the panel by ("GPU hardware").
    pub keywords: String,
}

impl SettingPanel {
    /// Whether every word of the search `query` is in the panel's text.
    pub fn matches(&self, query: &str) -> bool {
        let mut hay = format!(
            "{} {} {} {}",
            self.title, self.intro, self.footer, self.keywords
        );
        for r in &self.rows {
            hay.push(' ');
            hay.push_str(&r.label);
            hay.push(' ');
            hay.push_str(&r.value);
            for l in &r.lines {
                hay.push(' ');
                hay.push_str(l);
            }
        }
        let hay = fold_for_search(&hay);
        let q = fold_for_search(query);
        let mut words = q.split_whitespace().peekable();
        words.peek().is_some() && words.all(|w| hay.contains(w))
    }
}

/// What a panel's buttons ask the application to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PanelEvent {
    /// The button of the row with this [`PanelRow::id`].
    Row(String),
    /// The panel's action, confirmed.
    Action,
    /// The checkbox of the row with this id.
    Check(String),
    /// The disclosure arrow of the row with this id.
    Expand(String),
}

impl Chrome {
    /// Sets (or with `None` removes) the panel under the setting `after`. Returns whether
    /// anything changed (redraw then). A confirm in progress survives updates.
    pub fn set_setting_panel(&mut self, after: &'static str, panel: Option<SettingPanel>) -> bool {
        let changed = self.setting_panels.get(after) != panel.as_ref();
        match panel {
            Some(p) => {
                self.setting_panels.insert(after, p);
            }
            None => {
                self.setting_panels.remove(after);
            }
        }
        changed
    }

    pub(crate) fn panel_click(&mut self, s: &SettingsTarget) -> Option<crate::Command> {
        match s {
            SettingsTarget::PanelButton(k, i) => {
                let id = self.setting_panels.get(k)?.rows.get(*i)?.id.clone();
                Some(crate::Command::SettingPanel(k, PanelEvent::Row(id)))
            }
            SettingsTarget::PanelCheck(k, i) => {
                let id = self.setting_panels.get(k)?.rows.get(*i)?.id.clone();
                Some(crate::Command::SettingPanel(k, PanelEvent::Check(id)))
            }
            SettingsTarget::PanelExpand(k, i) => {
                let id = self.setting_panels.get(k)?.rows.get(*i)?.id.clone();
                Some(crate::Command::SettingPanel(k, PanelEvent::Expand(id)))
            }
            SettingsTarget::PanelAction(k) => {
                self.settings.panel_confirm = Some(k);
                None
            }
            SettingsTarget::PanelConfirm(k) => {
                self.settings.panel_confirm = None;
                Some(crate::Command::SettingPanel(k, PanelEvent::Action))
            }
            SettingsTarget::PanelCancel(_) => {
                self.settings.panel_confirm = None;
                None
            }
            _ => None,
        }
    }
}

/// Lays out and draws the panel under setting `key` from `y` (content coordinates already
/// scrolled); returns its height. Drawn and hit-tested only where it is inside `body`.
pub(crate) fn draw(
    ch: &Chrome,
    cx: &mut Ctx,
    key: &'static str,
    body: Rect,
    pad: f32,
    ctrl_w: f32,
    y: f32,
) -> f32 {
    let Some(p) = ch.setting_panels.get(key) else {
        return 0.0;
    };
    let confirming = ch.settings.panel_confirm == Some(key);
    let x = body.x + pad;
    let w = body.w - pad * 2.0;
    let text_w = (w - ctrl_w - 20.0).max(80.0);
    let visible = |r: Rect| r.bottom() > body.y && r.y < body.bottom();
    let title_s = cx.style(metrics::FONT_BODY).weight(650.0);
    let label_s = cx.style(metrics::FONT_BODY).weight(520.0);
    let line_s = cx.style(metrics::FONT_SMALL);
    let bh = cx.u(30.0);
    let mut cy = y + 16.0;

    // Heading and intro (none when both are empty: the rows go on from the setting).
    let headless = p.title.is_empty() && p.intro.is_empty();
    let tb = cx.layout(&p.title, title_s, Some(w));
    let ib = (!p.intro.is_empty()).then(|| cx.layout(&p.intro, line_s, Some(w)));
    let head_h = if headless {
        0.0
    } else {
        tb.height + ib.as_ref().map_or(0.0, |b| b.height + 3.0)
    };
    let head = Rect::new(x, cy, w, head_h);
    if !headless && visible(head) {
        cx.block_top(
            &tb,
            cx.m_in(Rect::new(x, cy, w, tb.height), head),
            HAlign::Start,
            cx.pal.text,
        );
        if let Some(ib) = &ib {
            let r = Rect::new(x, cy + tb.height + 3.0, w, ib.height);
            cx.block_top(ib, cx.m_in(r, head), HAlign::Start, cx.pal.text_muted);
        }
        cx.hit(head, Target::Settings(SettingsTarget::PanelHeading(key)));
    }
    cy += if headless { 0.0 } else { head_h + 8.0 };

    // Rows.
    for (i, r) in p.rows.iter().enumerate() {
        // Indent, disclosure arrow and tick box come before the text.
        let step = cx.u(22.0);
        let arrow_x = x + f32::from(r.indent) * step;
        let box_x = arrow_x + if r.expand.is_some() { step } else { 0.0 };
        let lead = box_x - x + if r.check.is_some() { cx.u(26.0) } else { 0.0 };
        let (x, text_w) = (x + lead, (text_w - lead).max(80.0));
        let lb = cx.layout(&r.label, label_s, Some(text_w));
        let lines: Vec<_> = r
            .lines
            .iter()
            .map(|l| cx.layout(l, line_s, Some(text_w)))
            .collect();
        let db = (!r.detail.is_empty()).then(|| cx.layout(&r.detail, line_s, Some(text_w)));
        let vb = cx.layout(&r.value, label_s, Some(ctrl_w));
        let bb = r
            .button
            .as_ref()
            .map(|b| cx.layout(b, cx.style(metrics::FONT_LABEL).weight(550.0), None));
        let text_h = lb.height
            + lines.iter().map(|b| b.height + 3.0).sum::<f32>()
            + db.as_ref().map_or(0.0, |b| b.height + 3.0);
        let ctrl_h = vb.height + bb.as_ref().map_or(0.0, |_| 8.0 + bh);
        let rh = text_h.max(ctrl_h).max(cx.u(32.0)) + 22.0;
        let row = Rect::new(x - lead, cy, w, rh);
        if visible(row) {
            // The row itself is one accessible element: name, value and the reasons.
            cx.hit(
                Rect::new(row.x, row.y, row.w - ctrl_w, row.h),
                Target::Settings(SettingsTarget::PanelRow(key, i)),
            );
            lead_controls(cx, key, i, r, row, (arrow_x, box_x), lb.height, x - row.x);
            let mut ty = row.y + 11.0;
            cx.block_top(
                &lb,
                cx.m_in(Rect::new(x, ty, text_w, lb.height), row),
                HAlign::Start,
                cx.pal.text,
            );
            ty += lb.height + 3.0;
            for (j, b) in lines.iter().enumerate() {
                let c = if r.attention && j == 0 {
                    cx.pal.accent
                } else {
                    cx.pal.text_muted
                };
                cx.block_top(
                    b,
                    cx.m_in(Rect::new(x, ty, text_w, b.height), row),
                    HAlign::Start,
                    c,
                );
                ty += b.height + 3.0;
            }
            if let Some(db) = &db {
                let dr = cx.m_in(Rect::new(x, ty, text_w, db.height), row);
                cx.block_top(db, dr, HAlign::Start, cx.pal.text_faint);
            }
            let ctrl_x = row.right() - ctrl_w;
            let mut vy = row.y + 11.0;
            let vc = if r.attention {
                cx.pal.accent
            } else {
                cx.pal.text
            };
            let vr = cx.m_in(Rect::new(ctrl_x, vy, ctrl_w, vb.height), row);
            cx.block_top(&vb, vr, HAlign::End, vc);
            vy += vb.height + 8.0;
            if let Some(bb) = &bb {
                let tgt = Target::Settings(SettingsTarget::PanelButton(key, i));
                let bw = (bb.width + 24.0).min(ctrl_w);
                let br = cx.m_in(Rect::new(row.right() - bw, vy, bw, bh), row);
                button(cx, bb, br, tgt, false);
            }
            cx.dl.rect(
                Rect::new(row.x, row.bottom() - 1.0, row.w, 1.0),
                0.0,
                cx.pal.separator,
            );
        }
        cy += rh;
    }

    // Footer and the action (or its confirmation).
    let qb = confirming
        .then(|| {
            p.action
                .as_ref()
                .map(|a| cx.layout(&a.question, line_s.weight(550.0), Some(w)))
        })
        .flatten();
    let bs = cx.style(metrics::FONT_LABEL).weight(550.0);
    let buttons: Vec<(
        std::sync::Arc<specular_gfx::TextBlock>,
        SettingsTarget,
        bool,
    )> = match (&p.action, confirming) {
        (Some(a), false) => vec![(
            cx.layout(&a.label, bs, None),
            SettingsTarget::PanelAction(key),
            false,
        )],
        (Some(a), true) => vec![
            (
                cx.layout(&a.cancel, bs, None),
                SettingsTarget::PanelCancel(key),
                false,
            ),
            (
                cx.layout(&a.confirm, bs, None),
                SettingsTarget::PanelConfirm(key),
                true,
            ),
        ],
        (None, _) => Vec::new(),
    };
    // The footer text sits beside the buttons, or above them when that leaves too little room.
    let buttons_w: f32 = buttons.iter().map(|(b, ..)| b.width + 24.0 + 8.0).sum();
    let beside_w = w - buttons_w - 12.0;
    let below = !buttons.is_empty() && !p.footer.is_empty() && beside_w < cx.u(200.0);
    let foot_w = if below || buttons.is_empty() {
        w
    } else {
        beside_w
    };
    let fb = (!p.footer.is_empty()).then(|| cx.layout(&p.footer, line_s, Some(foot_w)));
    let foot_text_h = fb.as_ref().map_or(0.0, |b| b.height);
    let q_h = qb.as_ref().map_or(0.0, |b| b.height + 8.0);
    let buttons_h = if buttons.is_empty() { 0.0 } else { bh };
    let foot_h =
        q_h + if below {
            foot_text_h + 8.0 + buttons_h
        } else {
            foot_text_h.max(buttons_h)
        } + 22.0;
    let foot = Rect::new(x, cy, w, foot_h);
    if visible(foot) {
        let mut fy = cy + 11.0;
        if let Some(qb) = &qb {
            let qr = cx.m_in(Rect::new(x, fy, w, qb.height), foot);
            cx.block_top(qb, qr, HAlign::Start, cx.pal.text);
            fy += qb.height + 8.0;
        }
        if let Some(fb) = &fb {
            let fr = cx.m_in(Rect::new(x, fy, foot_w, fb.height), foot);
            cx.block_top(fb, fr, HAlign::Start, cx.pal.text_faint);
            cx.hit(fr, Target::Settings(SettingsTarget::PanelFooter(key)));
            if below {
                fy += fb.height + 8.0;
            }
        }
        let mut bx = foot.right();
        for (b, t, primary) in buttons.iter().rev() {
            let bw = (b.width + 24.0).min(w);
            bx -= bw;
            let br = cx.m_in(Rect::new(bx, fy, bw, bh), foot);
            button(cx, b, br, Target::Settings(t.clone()), *primary);
            bx -= 8.0;
        }
    }
    cy += foot_h;
    cy - y
}

/// The disclosure arrow and tick box at the start of a row, with their click areas (the tick
/// box takes the label too; a row with only an arrow opens and closes from its label).
#[allow(clippy::too_many_arguments)]
fn lead_controls(
    cx: &mut Ctx,
    key: &'static str,
    i: usize,
    r: &PanelRow,
    row: Rect,
    (arrow_x, box_x): (f32, f32),
    label_h: f32,
    lead: f32,
) {
    let mid = row.y + 11.0 + label_h * 0.5;
    let size = cx.u(16.0);
    let label_end = (row.w * 0.6).max(lead + 80.0);
    if let Some(open) = r.expand {
        let ar = cx.m_in(Rect::new(arrow_x, mid - size * 0.5, size, size), row);
        let icon = if open {
            Icon::ChevronDown
        } else if cx.rtl {
            Icon::ChevronLeft
        } else {
            Icon::ChevronRight
        };
        draw_icon(cx.dl, icon, ar, cx.pal.text_muted, 1.8, false);
        let tgt = Target::Settings(SettingsTarget::PanelExpand(key, i));
        let hr = if r.check.is_some() {
            Rect::new(arrow_x - 4.0, row.y, box_x - arrow_x + 2.0, row.h)
        } else {
            Rect::new(row.x, row.y, label_end, row.h)
        };
        cx.hit(cx.m_in(hr, row), tgt);
    }
    if let Some(state) = r.check {
        let br = cx.m_in(Rect::new(box_x, mid - size * 0.5, size, size), row);
        let on = state != CheckState::Off;
        let (fill, border) = if on {
            (cx.pal.accent, cx.pal.accent)
        } else {
            (cx.pal.surface_raised, cx.pal.text_muted)
        };
        cx.dl.rect_border(br, cx.u(4.0), fill, border, 1.0);
        let mark = match state {
            CheckState::On => Some(Icon::Check),
            CheckState::Mixed => Some(Icon::Minus),
            CheckState::Off => None,
        };
        if let Some(m) = mark {
            draw_icon(
                cx.dl,
                m,
                br.centered(size * 0.8, size * 0.8),
                cx.pal.accent_text,
                2.0,
                false,
            );
        }
        let hr = Rect::new(box_x - 4.0, row.y, label_end - (box_x - row.x) + 4.0, row.h);
        cx.hit(
            cx.m_in(hr, row),
            Target::Settings(SettingsTarget::PanelCheck(key, i)),
        );
    }
}

fn button(cx: &mut Ctx, b: &specular_gfx::TextBlock, r: Rect, tgt: Target, primary: bool) {
    let hover = cx.is_hover(&tgt);
    let (fill, text) = if primary {
        (
            if hover {
                cx.pal.accent_fill.with_alpha(0.85)
            } else {
                cx.pal.accent_fill
            },
            cx.pal.accent_text,
        )
    } else {
        (
            if hover {
                cx.pal.surface_pressed
            } else {
                cx.pal.surface_raised
            },
            cx.pal.text,
        )
    };
    cx.dl
        .rect_border(r, metrics::RADIUS_SM + 1.0, fill, cx.pal.separator, 1.0);
    cx.dl.push_clip(r);
    cx.block_in(b, r, HAlign::Center, text);
    cx.dl.pop_clip();
    cx.hit(r, tgt);
}

/// Accessible name, value and description of a panel element; the bool says whether it can
/// be clicked.
pub(crate) fn describe(n: &mut accesskit::Node, s: &SettingsTarget, ch: &Chrome) -> Option<bool> {
    use accesskit::Role;
    match s {
        SettingsTarget::PanelHeading(k) => {
            let p = ch.setting_panels.get(k)?;
            n.set_role(Role::Heading);
            n.set_level(3);
            n.set_label(p.title.clone());
            if !p.intro.is_empty() {
                n.set_description(p.intro.clone());
            }
            Some(false)
        }
        SettingsTarget::PanelRow(k, i) => {
            let r = ch.setting_panels.get(k)?.rows.get(*i)?;
            n.set_role(Role::ListItem);
            n.set_label(r.label.clone());
            n.set_value(r.value.clone());
            let mut d = r.lines.join(" ");
            if !r.detail.is_empty() {
                if !d.is_empty() {
                    d.push(' ');
                }
                d.push_str(&r.detail);
            }
            if !d.is_empty() {
                n.set_description(d);
            }
            Some(false)
        }
        SettingsTarget::PanelFooter(k) => {
            n.set_role(Role::Label);
            n.set_value(ch.setting_panels.get(k)?.footer.clone());
            Some(false)
        }
        SettingsTarget::PanelButton(k, i) => {
            let r = ch.setting_panels.get(k)?.rows.get(*i)?;
            n.set_label(format!("{} — {}", r.button.clone()?, r.label));
            Some(true)
        }
        SettingsTarget::PanelCheck(k, i) => {
            let r = ch.setting_panels.get(k)?.rows.get(*i)?;
            n.set_role(Role::CheckBox);
            n.set_label(r.label.clone());
            if !r.value.is_empty() {
                n.set_value(r.value.clone());
            }
            if !r.lines.is_empty() {
                n.set_description(r.lines.join(" "));
            }
            n.set_toggled(match r.check? {
                CheckState::Off => accesskit::Toggled::False,
                CheckState::On => accesskit::Toggled::True,
                CheckState::Mixed => accesskit::Toggled::Mixed,
            });
            Some(true)
        }
        SettingsTarget::PanelExpand(k, i) => {
            let r = ch.setting_panels.get(k)?.rows.get(*i)?;
            n.set_label(r.label.clone());
            n.set_expanded(r.expand?);
            Some(true)
        }
        SettingsTarget::PanelAction(k) => {
            n.set_label(ch.setting_panels.get(k)?.action.as_ref()?.label.clone());
            Some(true)
        }
        SettingsTarget::PanelConfirm(k) => {
            let a = ch.setting_panels.get(k)?.action.as_ref()?;
            n.set_label(a.confirm.clone());
            n.set_description(a.question.clone());
            Some(true)
        }
        SettingsTarget::PanelCancel(k) => {
            n.set_label(ch.setting_panels.get(k)?.action.as_ref()?.cancel.clone());
            Some(true)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accesskit::{Node, TreeUpdate};
    use crate::scrollbar::Area;
    use crate::{Command, HudStats, Modal, Overlays, ViewerState};
    use specular_gfx::{DrawList, Text};
    use specular_input::{ActionState, ImageState, Keymap};
    use specular_settings::{Settings, SettingsStore};
    use specular_theme::Palette;

    fn with_state<R>(f: impl FnOnce(&ViewerState) -> R) -> R {
        let hud = HudStats::default();
        let keymap = Keymap::default();
        let store = SettingsStore::in_memory();
        let settings = Settings::default();
        let st = ViewerState {
            actions: ActionState {
                comparing: false,
                compare_images: 0,
                volume: false,
                mpr: false,
                image: ImageState::Ready,
                count: 1,
                gallery_files: 0,
                animated: false,
                video: false,
                sound: false,
                sound_file: false,
                in_archive: false,
                has_alpha: false,
                gallery_views: None,
                medical: None,
                dicom: false,
                series: 0,
                presentation_states: 0,
                science: None,
                debayer: false,
                annotations: false,
                stain: None,
                stain_region: false,
                slide_annotations: false,
                thermal_visible: false,
                drawing: false,
                mesh: false,
                points: false,
                molecule: false,
                tracts: false,
                relight: false,
                map: None,
                import_sources: false,
                pages: 0,
                rotatable: false,
                wallpaper: false,
                share: false,
                email: false,
                profile: specular_input::Profile::UNRESTRICTED,
            },
            has_image: true,
            index: 0,
            count: 1,
            zoom: 1.0,
            at_fit: true,
            fullscreen: false,
            file_name: Some("beach.jpg"),
            dims: Some((4000, 3000)),
            loading: false,
            error: None,
            truncated: false,
            metadata: None,
            hud: &hud,
            keymap: &keymap,
            store: &store,
            settings: &settings,
            overlays: Overlays::default(),
            playback: None,
            motion: None,
            spread: None,
            pages: None,
        };
        f(&st)
    }

    fn chrome() -> Chrome {
        let mut c = Chrome::new();
        c.set_accessibility(true);
        c.poke(1.0);
        c
    }

    /// Draws a few frames (the sheet fades in) at `w`×`h`; returns the hit targets.
    fn draw_hits(ch: &mut Chrome, st: &ViewerState, w: f32, h: f32) -> Vec<(Rect, Target)> {
        ch.set_size(w, h);
        let mut text = Text::new();
        let mut dl = DrawList::new(1.0);
        for i in 0..40 {
            dl.clear(1.0);
            ch.draw(
                &mut dl,
                &mut text,
                st,
                &Palette::dark(),
                1.0 + i as f64 * 0.016,
            );
        }
        ch.hits.clone()
    }

    fn tree(ch: &mut Chrome, st: &ViewerState) -> TreeUpdate {
        let _ = draw_hits(ch, st, ch.size.0, ch.size.1);
        ch.accessibility_tree(st, "beach.jpg", 1.0)
    }

    fn find_node<'a>(t: &'a TreeUpdate, label: &str) -> Option<&'a Node> {
        t.nodes
            .iter()
            .map(|(_, n)| n)
            .find(|n| n.label() == Some(label))
    }

    fn panel() -> SettingPanel {
        SettingPanel {
            title: "How Specular runs on this computer".into(),
            intro: "Measured on this computer.".into(),
            rows: vec![
                PanelRow {
                    id: "jpeg".into(),
                    label: "Opening JPEG photos".into(),
                    value: "Processor".into(),
                    lines: vec!["The hardware decoder was turned off after an error.".into()],
                    detail: "Processor 14 ms · 40 measurements".into(),
                    attention: true,
                    button: Some("Try again".into()),
                    ..Default::default()
                },
                PanelRow {
                    id: "colour".into(),
                    label: "Colour conversion".into(),
                    value: "Graphics processor".into(),
                    lines: vec!["Measured 2× faster.".into()],
                    detail: String::new(),
                    attention: false,
                    button: None,
                    ..Default::default()
                },
            ],
            footer: "Measured for: Radeon 860M · Mesa · Ryzen".into(),
            action: Some(PanelAction {
                label: "Measure again".into(),
                question: "Forget these measurements?".into(),
                confirm: "Forget measurements".into(),
                cancel: "Cancel".into(),
            }),
            keywords: "GPU hardware".into(),
        }
    }

    #[test]
    fn search_matches_text_and_keywords() {
        let p = panel();
        assert!(p.matches("gpu"));
        assert!(p.matches("HARDWARE decoder"));
        assert!(p.matches("jpeg"));
        assert!(p.matches("radeon"));
        assert!(!p.matches("gpu banana"));
        assert!(!p.matches("   "));
    }

    #[test]
    fn set_reports_changes() {
        let mut ch = Chrome::new();
        assert!(ch.set_setting_panel("performance.processing", Some(panel())));
        assert!(!ch.set_setting_panel("performance.processing", Some(panel())));
        assert!(ch.set_setting_panel("performance.processing", None));
        assert!(!ch.set_setting_panel("performance.processing", None));
    }

    /// The Performance section with the panel: rows, buttons and the confirm flow are hit
    /// targets with accessible names, inside the sheet, one under another.
    #[test]
    fn panel_rows_buttons_and_confirm() {
        with_state(|st| {
            let mut ch = chrome();
            ch.set_setting_panel("performance.processing", Some(panel()));
            ch.open_settings_at("performance");
            let find = |hits: &[(Rect, Target)], t: &SettingsTarget| {
                hits.iter()
                    .find(|(_, h)| *h == Target::Settings(t.clone()))
                    .map(|(r, _)| *r)
            };
            let k = "performance.processing";
            // The sheet is at most 660 px tall: the top of the panel first, then scrolled.
            let hits = draw_hits(&mut ch, st, 1280.0, 2400.0);
            let heading = find(&hits, &SettingsTarget::PanelHeading(k)).expect("heading");
            let row0 = find(&hits, &SettingsTarget::PanelRow(k, 0)).expect("row 0");
            let retry = find(&hits, &SettingsTarget::PanelButton(k, 0)).expect("retry button");
            // The panel sits right under "Where images are processed".
            let processing = find(&hits, &SettingsTarget::Dropdown(k)).expect("processing control");
            assert!(heading.y > processing.y);
            assert!(heading.bottom() <= row0.y);
            assert!(
                retry.x > row0.right() - 1.0,
                "the button is in the control column"
            );
            let scroll = 400.0;
            ch.settings.scroll = scroll;
            let hits = draw_hits(&mut ch, st, 1280.0, 2400.0);
            assert_eq!(ch.settings.scroll, scroll);
            let row1 = find(&hits, &SettingsTarget::PanelRow(k, 1)).expect("row 1");
            assert!(find(&hits, &SettingsTarget::PanelButton(k, 1)).is_none());
            let measure = find(&hits, &SettingsTarget::PanelAction(k)).expect("measure again");
            assert!(row0.bottom() - scroll <= row1.y + 0.5);
            assert!(row1.bottom() <= measure.y);
            // Whatever came after the processing setting is now below the panel.
            let after =
                hits.iter()
                    .filter_map(|(r, h)| match h {
                        Target::Settings(
                            SettingsTarget::Dropdown(k2) | SettingsTarget::Toggle(k2),
                        ) if *k2 != k && r.y > measure.y => Some(r.y),
                        _ => None,
                    })
                    .fold(f32::INFINITY, f32::min);
            assert!(
                after.is_finite() && after > measure.bottom(),
                "{after} vs {measure:?}"
            );

            // Accessible (of what is on screen, as for the other rows): the row reads name,
            // value and why.
            let t = tree(&mut ch, st);
            let row = find_node(&t, "Opening JPEG photos").expect("row node");
            assert_eq!(row.value(), Some("Processor"));
            assert!(
                row.description()
                    .unwrap()
                    .contains("turned off after an error")
            );
            assert!(find_node(&t, "Try again — Opening JPEG photos").is_some());
            assert!(find_node(&t, "Measure again").is_some());

            // Retry sends the row's id; Measure again asks first.
            assert_eq!(
                ch.panel_click(&SettingsTarget::PanelButton(k, 0)),
                Some(Command::SettingPanel(k, PanelEvent::Row("jpeg".into())))
            );
            assert_eq!(ch.panel_click(&SettingsTarget::PanelAction(k)), None);
            let hits = draw_hits(&mut ch, st, 1280.0, 2400.0);
            let has =
                |t: SettingsTarget| hits.iter().any(|(_, h)| *h == Target::Settings(t.clone()));
            assert!(has(SettingsTarget::PanelConfirm(k)) && has(SettingsTarget::PanelCancel(k)));
            assert!(!has(SettingsTarget::PanelAction(k)));
            // Updates while confirming keep the question open.
            let mut p2 = panel();
            p2.rows[1].value = "Processor".into();
            assert!(ch.set_setting_panel(k, Some(p2)));
            assert_eq!(ch.panel_click(&SettingsTarget::PanelCancel(k)), None);
            assert_eq!(ch.panel_click(&SettingsTarget::PanelAction(k)), None);
            assert_eq!(
                ch.panel_click(&SettingsTarget::PanelConfirm(k)),
                Some(Command::SettingPanel(k, PanelEvent::Action))
            );
            assert_eq!(ch.modal, Some(Modal::Settings));
            let hits = draw_hits(&mut ch, st, 1280.0, 2400.0);
            assert!(
                hits.iter()
                    .any(|(_, h)| *h == Target::Settings(SettingsTarget::PanelAction(k)))
            );
        });
    }

    /// Rows with a disclosure arrow and tick boxes (Integration → "More file types"): the
    /// arrow and the box are separate targets with their own accessible state, children are
    /// indented, and both send the row's id back.
    #[test]
    fn panel_checks_and_disclosure() {
        with_state(|st| {
            let mut ch = chrome();
            let k = "performance.processing";
            let row = |id: &str, label: &str, check, expand, indent| PanelRow {
                id: id.into(),
                label: label.into(),
                check,
                expand,
                indent,
                ..Default::default()
            };
            let p = SettingPanel {
                rows: vec![
                    row("more", "More file types", None, Some(true), 0),
                    row(
                        "g:cad",
                        "CAD and GIS",
                        Some(CheckState::Mixed),
                        Some(true),
                        1,
                    ),
                    row("t:hpgl", "HP-GL plots", Some(CheckState::On), None, 3),
                    row(
                        "t:gds",
                        "GDSII chip layouts",
                        Some(CheckState::Off),
                        None,
                        3,
                    ),
                ],
                ..Default::default()
            };
            ch.set_setting_panel(k, Some(p));
            ch.open_settings_at(k);
            let hits = draw_hits(&mut ch, st, 1280.0, 2400.0);
            let find = |t: SettingsTarget| {
                hits.iter()
                    .rev()
                    .find(|(_, h)| *h == Target::Settings(t.clone()))
                    .map(|(r, _)| *r)
            };
            let more = find(SettingsTarget::PanelExpand(k, 0)).expect("more arrow");
            assert!(find(SettingsTarget::PanelCheck(k, 0)).is_none());
            let g_arrow = find(SettingsTarget::PanelExpand(k, 1)).expect("group arrow");
            let g_box = find(SettingsTarget::PanelCheck(k, 1)).expect("group box");
            let item = find(SettingsTarget::PanelCheck(k, 2)).expect("item box");
            assert!(find(SettingsTarget::PanelExpand(k, 2)).is_none());
            assert!(g_arrow.x > more.x, "groups are indented");
            assert!(g_box.x > g_arrow.x, "the box comes after the arrow");
            assert!(item.x > g_box.x, "types are indented under their group");
            assert!(g_box.y >= more.bottom() - 0.5 && item.y >= g_box.bottom() - 0.5);
            // A click on the label hits the box, not the row behind it.
            let c = (item.x + 30.0, item.y + item.h * 0.5);
            let top = hits
                .iter()
                .rev()
                .find(|(r, _)| r.contains(c))
                .map(|h| h.1.clone());
            assert_eq!(
                top,
                Some(Target::Settings(SettingsTarget::PanelCheck(k, 2)))
            );

            let t = tree(&mut ch, st);
            let boxes: Vec<_> = t
                .nodes
                .iter()
                .map(|(_, n)| n)
                .filter(|n| n.role() == accesskit::Role::CheckBox)
                .map(|n| (n.label().unwrap_or_default().to_string(), n.toggled()))
                .collect();
            let has = |l: &str, s| boxes.contains(&(l.to_string(), Some(s)));
            assert!(has("CAD and GIS", accesskit::Toggled::Mixed), "{boxes:?}");
            assert!(has("HP-GL plots", accesskit::Toggled::True));
            assert!(has("GDSII chip layouts", accesskit::Toggled::False));
            assert!(t.nodes.iter().any(|(_, n)| {
                n.label() == Some("More file types") && n.is_expanded() == Some(true)
            }));

            assert_eq!(
                ch.panel_click(&SettingsTarget::PanelCheck(k, 2)),
                Some(Command::SettingPanel(k, PanelEvent::Check("t:hpgl".into())))
            );
            assert_eq!(
                ch.panel_click(&SettingsTarget::PanelExpand(k, 1)),
                Some(Command::SettingPanel(k, PanelEvent::Expand("g:cad".into())))
            );
        });
    }

    /// Opening Settings at the setting scrolls it, and the panel under it, into view.
    #[test]
    fn open_at_the_setting_scrolls_to_it() {
        with_state(|st| {
            let mut ch = chrome();
            ch.set_setting_panel("performance.processing", Some(panel()));
            ch.open_settings_at("performance.processing");
            let hits = draw_hits(&mut ch, st, 1280.0, 2400.0);
            let k = "performance.processing";
            let at = |t: SettingsTarget| {
                hits.iter()
                    .find(|(_, h)| *h == Target::Settings(t.clone()))
                    .map(|(r, _)| *r)
            };
            assert!(ch.settings.scroll > 0.0);
            let control = at(SettingsTarget::Dropdown(k)).expect("processing control");
            let row0 = at(SettingsTarget::PanelRow(k, 0)).expect("first row in view");
            assert!(control.y < row0.y);
        });
    }

    #[test]
    fn search_finds_the_panel_under_its_setting() {
        with_state(|st| {
            let mut ch = chrome();
            ch.set_setting_panel("performance.processing", Some(panel()));
            ch.open_settings_at("general");
            ch.settings.type_text("GPU");
            let hits = draw_hits(&mut ch, st, 1280.0, 2400.0);
            let k = "performance.processing";
            assert!(
                hits.iter()
                    .any(|(_, h)| *h == Target::Settings(SettingsTarget::PanelRow(k, 0)))
            );
            ch.settings.search.clear();
            ch.settings.type_text("radeon");
            let hits = draw_hits(&mut ch, st, 1280.0, 2400.0);
            assert!(
                hits.iter()
                    .any(|(_, h)| *h == Target::Settings(SettingsTarget::PanelHeading(k)))
            );
            ch.settings.search.clear();
            ch.settings.type_text("thumbnail");
            let hits = draw_hits(&mut ch, st, 1280.0, 2400.0);
            assert!(
                !hits
                    .iter()
                    .any(|(_, h)| *h == Target::Settings(SettingsTarget::PanelHeading(k)))
            );
        });
    }

    /// Small windows and right-to-left layouts keep every element inside the sheet.
    #[test]
    fn narrow_layout_stays_inside() {
        with_state(|st| {
            let mut ch = chrome();
            ch.set_setting_panel("performance.processing", Some(panel()));
            ch.open_settings_at("performance");
            let hits = draw_hits(&mut ch, st, 520.0, 4000.0);
            for (r, h) in &hits {
                if matches!(h, Target::Settings(s) if s.is_panel()) {
                    assert!(r.x >= 0.0 && r.right() <= 520.0, "{h:?} {r:?}");
                    assert!(r.w > 0.0 && r.h > 0.0, "{h:?} {r:?}");
                }
            }
        });
    }

    /// A window too short for every section keeps the rows full height and scrolls the
    /// sidebar instead: the wheel over it scrolls the list, and the section chosen from the
    /// keyboard comes into view.
    #[test]
    fn short_window_scrolls_the_sidebar() {
        with_state(|st| {
            let mut ch = chrome();
            ch.open_settings_at("general");
            let rows = |hits: &[(Rect, Target)]| -> Vec<(usize, Rect)> {
                hits.iter()
                    .filter_map(|(r, t)| match t {
                        Target::Settings(SettingsTarget::Section(i)) => Some((*i, *r)),
                        _ => None,
                    })
                    .collect()
            };
            let tall = rows(&draw_hits(&mut ch, st, 900.0, 2000.0));
            let n = ch.sections().len();
            assert_eq!(tall.len(), n, "all sections in a tall window");
            let pitch = tall[1].1.y - tall[0].1.y;
            let short = rows(&draw_hits(&mut ch, st, 900.0, 360.0));
            assert!(short.len() < n, "{} of {n} shown", short.len());
            let full: Vec<_> = short.iter().filter(|(_, r)| r.h >= pitch - 2.5).collect();
            assert!(
                full.len() >= 2 && full[1].1.y - full[0].1.y == pitch,
                "rows keep their height"
            );
            assert!(
                ch.bars.geometry(Area::SettingsNav).is_some(),
                "the sidebar has a scrollbar"
            );
            // The wheel over the sidebar scrolls the list, not the page.
            let side = ch.settings.side;
            let page = ch.settings.scroll;
            ch.input(
                &crate::Input::Wheel {
                    x: side.x + 20.0,
                    dx: 0.0,
                    y: side.bottom() - 40.0,
                    dy: -3.0,
                },
                st,
                2.0,
            );
            assert!(ch.settings.nav_scroll > 0.0);
            assert_eq!(ch.settings.scroll, page);
            // The last section, chosen from the keyboard, is drawn inside the list.
            ch.settings.nav_scroll = 0.0;
            ch.settings.section = n - 1;
            let last = rows(&draw_hits(&mut ch, st, 900.0, 360.0));
            let r = last
                .iter()
                .find(|(i, _)| *i == n - 1)
                .expect("last section drawn")
                .1;
            assert!(r.bottom() <= side.bottom(), "{r:?} inside {side:?}");
        });
    }

    /// Settings → Local AI: with nothing offered it says so and shows no feature switch;
    /// installed features get their switches, People its panel; hidden settings are not
    /// found by search either; the page is in the accessibility tree.
    #[test]
    fn local_ai_page_hides_what_is_not_installed() {
        use crate::{AiFeatureRow, AiPage, AiPerson, AiRowState};
        with_state(|st| {
            let mut ch = chrome();
            ch.set_ai_page(&AiPage::default());
            ch.open_settings_at("ai");
            let hits = draw_hits(&mut ch, st, 1280.0, 2400.0);
            let has = |hits: &[(Rect, Target)], t: SettingsTarget| {
                hits.iter().any(|(_, h)| *h == Target::Settings(t.clone()))
            };
            assert!(has(&hits, SettingsTarget::PanelRow("ai.pack", 0)));
            assert!(!has(&hits, SettingsTarget::Act("ai.pack")), "nothing to do");
            for k in ["ai.search", "ai.text", "ai.culling", "ai.people"] {
                assert!(!has(&hits, SettingsTarget::Toggle(k)), "{k}");
            }
            let t = tree(&mut ch, st);
            assert!(find_node(&t, "No models available in this build.").is_some());
            ch.settings.search = "people".into();
            let hits = draw_hits(&mut ch, st, 1280.0, 2400.0);
            assert!(!has(&hits, SettingsTarget::Toggle("ai.people")));

            ch.settings.search.clear();
            ch.set_ai_page(&AiPage {
                features: vec![AiFeatureRow {
                    feature: "people",
                    state: AiRowState::Installed { indexed: None },
                    size: 30_000_000,
                    licences: "MIT".into(),
                    models: "faces 1".into(),
                    ..Default::default()
                }],
                people: vec![AiPerson {
                    id: 7,
                    name: "Ana".into(),
                    faces: 3,
                }],
                gpu: None,
                face_grouping: None,
                text_indexing: None,
            });
            ch.open_settings_at("ai");
            let hits = draw_hits(&mut ch, st, 1280.0, 2400.0);
            assert!(has(&hits, SettingsTarget::Toggle("ai.people")));
            assert!(!has(&hits, SettingsTarget::Toggle("ai.search")));
            assert!(has(&hits, SettingsTarget::PanelButton("ai.people", 0)));
            assert!(has(&hits, SettingsTarget::PanelAction("ai.people")));
            assert!(has(&hits, SettingsTarget::PanelAction("ai.pack")));
            assert_eq!(
                ch.panel_click(&SettingsTarget::PanelButton("ai.people", 0)),
                Some(Command::SettingPanel(
                    "ai.people",
                    PanelEvent::Row("forget:7".into())
                ))
            );
            let t = tree(&mut ch, st);
            assert!(find_node(&t, "Forget — Ana").is_some());
            assert!(find_node(&t, "Delete all face data").is_some());
        });
    }
}
