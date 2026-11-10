//! The start screen: what the window shows with nothing open (at a start without a file, and
//! once the last image is closed). One line says what Specular is; three large entry points
//! open a file, browse a folder or show the library ("Your pictures"); the recent folders are
//! thumbnail cards; and "Things to try" is a row of small cards, each a feature with its key
//! from the keymap (a card whose action has no key, or that the profile hides, is left out).
//! Files dropped anywhere on it open as they do on the viewer.
//!
//! Every control is a Tab stop of its own, in reading order (Enter or Space presses it), and
//! the layout is mirrored right to left.

use std::path::PathBuf;

use specular_gfx::{Icon, Rect, TexId, draw_icon};
use specular_i18n::{I18n, t};
use specular_input::fixed::{self, GalleryCmd};
use specular_input::{Action, ActionState, Keymap, Mode};
use specular_settings::Origin;
use specular_theme::metrics;

use crate::ctx::{Ctx, HAlign, Target};
use crate::{Chrome, Command, Response, ViewerState};

/// A control of the start screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum WelcomeTarget {
    /// Open a file…
    Open,
    /// Browse a folder…
    Browse,
    /// Your pictures (the library).
    Pictures,
    /// A recent folder, by index in [`Welcome::recents`].
    Recent(usize),
    /// A "Things to try" card, by index in [`tips`].
    Tip(usize),
    /// "Hide these tips".
    HideTips,
}

/// What a control of the start screen asks the application to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WelcomeCommand {
    /// Pick a folder, then show it as a gallery.
    Browse,
    /// Show the library; while it has no folders, ask to add the Pictures folder first.
    Pictures,
    /// Show this folder as a gallery (a recent folder).
    Folder(PathBuf),
}

/// One recent folder card.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecentFolder {
    pub path: PathBuf,
    /// Its name (the last part of the path).
    pub name: String,
    /// Where it is (the folder it is in, shortened as the user knows it: `~/Pictures`).
    pub parent: String,
    /// One of its images, as a thumbnail (texture, width, height), once made.
    pub thumb: Option<(TexId, u32, u32)>,
}

/// What the application tells the start screen ([`Chrome::set_welcome`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Welcome {
    /// Most recent first; none: the row is left out.
    pub recents: Vec<RecentFolder>,
    /// The library has no folders yet: the name of the Pictures folder "Your pictures" offers
    /// to add.
    pub pictures_offer: Option<String>,
}

/// A "Things to try" card's feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TipKind {
    Gallery,
    Compare,
    Cull,
    Slideshow,
    Palette,
    Studies,
    DicomServers,
    DicomTags,
    WindowLevel,
    OmeroServers,
    Stretch,
    DrawingLayers,
}

/// What a card needs before its action can run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Needs {
    Nothing,
    Image,
    Gallery,
}

impl TipKind {
    /// The cards for everyone, in order.
    const GENERAL: [TipKind; 5] = [
        TipKind::Gallery,
        TipKind::Compare,
        TipKind::Cull,
        TipKind::Slideshow,
        TipKind::Palette,
    ];
    /// A profile's candidates: the first with a key is its card.
    const PROFILES: [&'static [TipKind]; 3] = [
        &[
            TipKind::Studies,
            TipKind::DicomServers,
            TipKind::DicomTags,
            TipKind::WindowLevel,
        ],
        &[TipKind::OmeroServers, TipKind::Stretch],
        &[TipKind::DrawingLayers],
    ];

    /// The id in its strings (`welcome-tip-<id>`, `welcome-tip-<id>-about`).
    pub fn id(self) -> &'static str {
        match self {
            TipKind::Gallery => "gallery",
            TipKind::Compare => "compare",
            TipKind::Cull => "cull",
            TipKind::Slideshow => "slideshow",
            TipKind::Palette => "palette",
            TipKind::Studies => "studies",
            TipKind::DicomServers => "dicom-servers",
            TipKind::DicomTags => "dicom-tags",
            TipKind::WindowLevel => "window-level",
            TipKind::OmeroServers => "omero-servers",
            TipKind::Stretch => "stretch",
            TipKind::DrawingLayers => "drawing-layers",
        }
    }

    /// The action the card runs, and the keymap mode its key is looked up in. Compare has
    /// none: its key is the gallery's own.
    fn action(self) -> Option<(Action, Mode)> {
        let v = Mode::Viewer;
        Some(match self {
            TipKind::Gallery => (Action::ToggleGallery, v),
            TipKind::Compare => return None,
            TipKind::Cull => (Action::ToggleCull, v),
            TipKind::Slideshow => (Action::Slideshow, v),
            TipKind::Palette => (Action::Palette, v),
            TipKind::Studies => (Action::Studies, v),
            TipKind::DicomServers => (Action::DicomServers, v),
            TipKind::DicomTags => (Action::DicomTags, v),
            TipKind::WindowLevel => (Action::WindowLevelTool, Mode::Medical),
            TipKind::OmeroServers => (Action::OmeroServers, v),
            TipKind::Stretch => (Action::ScienceStretch, Mode::Science),
            TipKind::DrawingLayers => (Action::DrawingLayers, Mode::Drawing),
        })
    }

    fn needs(self) -> Needs {
        match self {
            TipKind::Palette | TipKind::DicomServers | TipKind::OmeroServers => Needs::Nothing,
            TipKind::Compare => Needs::Gallery,
            _ => Needs::Image,
        }
    }

    pub fn title(self) -> String {
        I18n::global().tr(&format!("welcome-tip-{}", self.id()))
    }

    pub fn about(self) -> String {
        I18n::global().tr(&format!("welcome-tip-{}-about", self.id()))
    }
}

/// One "Things to try" card.
#[derive(Clone, Debug, PartialEq)]
pub struct Tip {
    pub kind: TipKind,
    /// Its key, as the keymap spells it.
    pub key: String,
    /// What pressing it runs; `None`: it says what to do first.
    pub run: Option<Action>,
}

/// The key of `action` in `mode` (the active modes' first for the viewer), when bound and
/// shown by the profile.
fn key_in(km: &Keymap, action: Action, mode: Mode) -> Option<String> {
    if mode == Mode::Viewer {
        return km.primary_key(action).map(|k| k.to_string());
    }
    if !km.shows(action) {
        return None;
    }
    km.bindings(mode)
        .iter()
        .find(|(_, a)| *a == action)
        .map(|(k, _)| k.to_string())
}

/// The gallery's key for Compare (its own table, not the keymap).
pub fn compare_key() -> Option<String> {
    fixed::GALLERY_KEYS
        .iter()
        .find(|l| l.cmd == GalleryCmd::Compare)
        .and_then(|l| l.listed_combos().next())
        .map(|c| c.to_string())
}

/// The "Things to try" cards: the general ones, then one per profile that is on, at most
/// six. A card whose action has no key or is hidden by the profile is left out.
pub fn tips(km: &Keymap, st: &ActionState, has_image: bool) -> Vec<Tip> {
    let make = |kind: TipKind| -> Option<Tip> {
        let (key, run) = match kind.action() {
            None => (compare_key()?, None),
            Some((a, mode)) => {
                if a.hidden(st).is_some() {
                    return None;
                }
                let key = key_in(km, a, mode)?;
                let ready = match kind.needs() {
                    Needs::Nothing => true,
                    Needs::Image => has_image,
                    Needs::Gallery => false,
                };
                (key, (ready && a.enabled(st)).then_some(a))
            }
        };
        Some(Tip { kind, key, run })
    };
    let extra: Vec<Tip> = TipKind::PROFILES
        .iter()
        .filter_map(|c| c.iter().find_map(|k| make(*k)))
        .collect();
    let mut out: Vec<Tip> = TipKind::GENERAL.iter().filter_map(|k| make(*k)).collect();
    out.truncate(6usize.saturating_sub(extra.len()).max(4));
    out.extend(extra);
    out.truncate(6);
    out
}

/// What pressing tip `t` says when it cannot run yet (one line).
pub fn tip_later(t: &Tip) -> String {
    match t.kind.needs() {
        Needs::Gallery => t!("welcome-tip-needs-gallery", key = t.key.clone()),
        _ => t!("welcome-tip-needs-image", key = t.key.clone()),
    }
}

impl Chrome {
    /// What the start screen shows besides its fixed parts: the recent folders and whether
    /// "Your pictures" offers to set the library up.
    pub fn set_welcome(&mut self, w: Welcome) {
        self.welcome = w;
    }

    /// Whether the start screen was on screen at the last draw.
    pub fn welcome_shown(&self) -> bool {
        self.welcome_drawn
    }

    /// The start screen's tips as they are now (`tips` for this state).
    pub(crate) fn welcome_tips(&self, st: &ViewerState) -> Vec<Tip> {
        if !st.settings.general.start_tips {
            return Vec::new();
        }
        tips(st.keymap, &st.actions, st.has_image)
    }

    /// A control of the start screen pressed.
    pub(crate) fn welcome_activate(&mut self, w: WelcomeTarget, st: &ViewerState) -> Response {
        match w {
            WelcomeTarget::Open => Response::cmd(Command::Action(Action::Open)),
            WelcomeTarget::Browse => Response::cmd(Command::Welcome(WelcomeCommand::Browse)),
            WelcomeTarget::Pictures => Response::cmd(Command::Welcome(WelcomeCommand::Pictures)),
            WelcomeTarget::Recent(i) => match self.welcome.recents.get(i) {
                Some(r) => Response::cmd(Command::Welcome(WelcomeCommand::Folder(r.path.clone()))),
                None => Response::used(),
            },
            WelcomeTarget::Tip(i) => {
                let Some(tip) = self.welcome_tips(st).into_iter().nth(i) else {
                    return Response::used();
                };
                match tip.run {
                    Some(a) => Response::cmd(Command::Action(a)),
                    None => {
                        let now = self.last_now;
                        self.toast(tip_later(&tip), None, now);
                        Response::used()
                    }
                }
            }
            WelcomeTarget::HideTips => {
                st.store.update(Origin::SettingsWindow, |s| {
                    s.general.start_tips = false;
                });
                let now = self.last_now;
                self.toast(t!("welcome-tips-hidden"), None, now);
                // The cards are gone: the keyboard goes back to the start.
                self.set_kbd(None);
                Response::used()
            }
        }
    }
}

/// The accessible name of a recent folder card.
pub(crate) fn recent_name(r: &RecentFolder) -> String {
    if r.parent.is_empty() {
        r.name.clone()
    } else {
        t!(
            "welcome-recent-name",
            name = r.name.clone(),
            parent = r.parent.clone()
        )
    }
}

/// The label and the line under it of a large entry point.
pub(crate) fn entry_texts(ch: &Chrome, w: WelcomeTarget) -> (String, String) {
    match w {
        WelcomeTarget::Open => (t!("welcome-open"), t!("welcome-open-about")),
        WelcomeTarget::Browse => (t!("welcome-browse"), t!("welcome-browse-about")),
        // No library yet: the card says what a click does (it asks to add the folder).
        _ => match &ch.welcome.pictures_offer {
            Some(f) => (
                t!("welcome-pictures-setup-title"),
                t!("welcome-pictures-setup", folder = f.clone()),
            ),
            None => (t!("welcome-pictures"), t!("welcome-pictures-about")),
        },
    }
}

/// A recent folder card's tooltip: where the folder is (`~/Pictures/Trips`), as its name
/// alone can be ambiguous.
pub(crate) fn recent_tooltip(ch: &Chrome, i: usize) -> Option<String> {
    let rf = ch.welcome.recents.get(i)?;
    let sep = std::path::MAIN_SEPARATOR;
    Some(if rf.parent.is_empty() {
        rf.name.clone()
    } else if rf.parent.ends_with(sep) {
        format!("{}{}", rf.parent, rf.name)
    } else {
        format!("{}{sep}{}", rf.parent, rf.name)
    })
}

const ENTRIES: [(WelcomeTarget, Icon); 3] = [
    (WelcomeTarget::Open, Icon::Image),
    (WelcomeTarget::Browse, Icon::Folder),
    (WelcomeTarget::Pictures, Icon::Grid),
];

/// Draws the start screen over the whole window (the toolbar is not shown with it).
pub(crate) fn draw(ch: &Chrome, cx: &mut Ctx, st: &ViewerState) {
    if ch.file_hover {
        draw_drop(cx);
        return;
    }
    let tips = ch.welcome_tips(st);
    let gap = cx.u(12.0);
    let col_w = cx.u(760.0).min(cx.w - 32.0).max(1.0);
    let col_x = ((cx.w - col_w) * 0.5).round();
    let top = 24.0f32;
    let bottom = cx.h - top;

    // The heading and the line under it.
    let title = cx.layout(
        &t!("welcome-title"),
        cx.style(metrics::FONT_LARGE * 1.4).weight(650.0),
        Some(col_w),
    );
    let tagline = cx.layout(
        &t!("welcome-tagline"),
        cx.style(metrics::FONT_BODY),
        Some(col_w),
    );
    let head_h = title.height + 6.0 + tagline.height;

    // The entry points: three side by side, else stacked.
    let side = col_w >= cx.u(170.0) * 3.0 + gap * 2.0;
    let entry_w = if side {
        ((col_w - gap * 2.0) / 3.0).floor()
    } else {
        col_w
    };
    let icon_s = cx.u(26.0);
    let pad = cx.u(14.0);
    let lstyle = cx.style(metrics::FONT_TITLE).weight(600.0);
    let astyle = cx.style(metrics::FONT_SMALL);
    let text_w = if side {
        entry_w - pad * 2.0
    } else {
        entry_w - pad * 3.0 - icon_s
    };
    // Compact (a short window): the icon beside the label, without the line under it.
    let ctext_w = (entry_w - pad * 3.0 - icon_s).max(1.0);
    let entries: Vec<_> = ENTRIES
        .iter()
        .map(|(w, icon)| {
            let (l, a) = entry_texts(ch, *w);
            let lb = cx.layout(&l, lstyle, Some(text_w));
            let ab = cx.layout(&a, astyle, Some(text_w));
            let lc = cx.layout(&l, lstyle, Some(ctext_w));
            (*w, *icon, lb, ab, lc)
        })
        .collect();
    let entry_h_full = entries
        .iter()
        .map(|(_, _, lb, ab, _)| {
            let text = lb.height + 4.0 + ab.height;
            if side {
                pad + icon_s + 10.0 + text + pad
            } else {
                (pad + text + pad).max(icon_s + pad * 2.0)
            }
        })
        .fold(0.0f32, f32::max);
    let entry_h_compact = entries
        .iter()
        .map(|(.., lc)| lc.height.max(icon_s) + pad * 1.5)
        .fold(0.0f32, f32::max);
    let entries_h = |compact: bool| {
        let h = if compact {
            entry_h_compact
        } else {
            entry_h_full
        };
        if side { h } else { h * 3.0 + gap * 2.0 }
    };
    let drop = cx.layout(&t!("welcome-drop"), astyle, Some(col_w));

    // The recent folders: one row of cards, as many as fit, as wide as fills the column with
    // that many (so the row ends where the others do; fewer folders leave the end empty).
    let fit = (((col_w + gap) / (cx.u(132.0) + gap)).floor().max(1.0) as usize).min(6);
    let card_w = ((col_w - gap * (fit as f32 - 1.0)) / fit as f32).floor();
    let thumb_h = (card_w * 0.75).round();
    let name_style = cx.style(metrics::FONT_SMALL).weight(500.0);
    let name_h = cx.u(18.0);
    let heading_style = cx.style(metrics::FONT_LABEL).weight(600.0);
    let heading_h = cx.u(20.0);
    let n_recent = ch.welcome.recents.len().min(fit);
    let recent_h = if n_recent > 0 {
        heading_h + 10.0 + thumb_h + 6.0 + name_h
    } else {
        0.0
    };

    // "Things to try": a grid of small cards.
    let tip_min = cx.u(220.0);
    let cols = (((col_w + gap) / (tip_min + gap)).floor() as usize).clamp(1, 3);
    let tip_w = ((col_w - gap * (cols as f32 - 1.0)) / cols as f32).floor();
    let tstyle = cx.style(metrics::FONT_LABEL).weight(600.0);
    let tip_pad = cx.u(10.0);
    let tip_text_w = (tip_w - tip_pad * 2.0).max(1.0);
    let tip_blocks: Vec<_> = tips
        .iter()
        .map(|tp| {
            let tb = cx.layout(&tp.kind.title(), tstyle, Some(tip_text_w));
            let ab = cx.layout(&tp.kind.about(), astyle, Some(tip_text_w));
            (tb, ab)
        })
        .collect();
    let tip_h = tip_blocks
        .iter()
        .map(|(tb, ab)| tip_pad + tb.height.max(cx.u(20.0)) + 4.0 + ab.height + tip_pad)
        .fold(0.0f32, f32::max);
    let rows = tips.len().div_ceil(cols);
    let tips_h = if tips.is_empty() {
        0.0
    } else {
        heading_h + 10.0 + rows as f32 * tip_h + (rows as f32 - 1.0).max(0.0) * gap
    };
    // With the tips hidden, the line about the keys and the palette instead.
    let hint = if st.settings.general.start_tips {
        None
    } else {
        crate::empty_hint(st.keymap).map(|h| cx.layout(&h, astyle, Some(col_w)))
    };

    let (sec_full, sec_compact) = (cx.u(28.0), cx.u(20.0));
    let section_of = move |compact: bool| if compact { sec_compact } else { sec_full };
    let total = |compact: bool, recents: bool, tips: bool| {
        let section = section_of(compact);
        let mut h = head_h + section + entries_h(compact);
        if !compact {
            h += 10.0 + drop.height;
        }
        if recents && recent_h > 0.0 {
            h += section + recent_h;
        }
        if tips && tips_h > 0.0 {
            h += section + tips_h;
        }
        if let Some(b) = &hint {
            h += section + b.height;
        }
        h
    };
    let avail = bottom - top;
    // What does not fit is made smaller (the entry points), then left out: the tips first,
    // then the recent folders.
    let (compact, show_recent, show_tips) = [
        (false, true, true),
        (true, true, true),
        (true, true, false),
        (true, false, true),
    ]
    .into_iter()
    .find(|(c, r, t)| total(*c, *r, *t) <= avail)
    .unwrap_or((true, false, false));
    let h = total(compact, show_recent, show_tips);
    let section = section_of(compact);
    let entry_h = if compact {
        entry_h_compact
    } else {
        entry_h_full
    };
    let mut y = (top + ((avail - h) * 0.5).max(0.0)).round();
    let col = Rect::new(col_x, y, col_w, h);

    // Heading.
    let r = cx.m_in(Rect::new(col_x, y, col_w, title.height), col);
    cx.block_top(&title, r, HAlign::Start, cx.pal.text);
    y += title.height + 6.0;
    let r = cx.m_in(Rect::new(col_x, y, col_w, tagline.height), col);
    cx.block_top(&tagline, r, HAlign::Start, cx.pal.text_muted);
    y += tagline.height + section;

    // Entry points.
    for (i, (w, icon, lb, ab, lc)) in entries.iter().enumerate() {
        let er = if side {
            Rect::new(col_x + i as f32 * (entry_w + gap), y, entry_w, entry_h)
        } else {
            Rect::new(col_x, y + i as f32 * (entry_h + gap), entry_w, entry_h)
        };
        let er = cx.m_in(er, col);
        let t = Target::Welcome(*w);
        let primary = *w == WelcomeTarget::Open;
        let hov = cx.is_hover(&t);
        let (fill, ink, muted) = if primary {
            let f = if hov {
                cx.pal.accent_fill.lerp(cx.pal.accent_text, 0.12)
            } else {
                cx.pal.accent_fill
            };
            // The line under the label in full white too: dimmed, it is too faint on the accent.
            (f, cx.pal.accent_text, cx.pal.accent_text)
        } else {
            let f = if hov {
                cx.pal.surface_hover
            } else {
                cx.pal.surface_raised
            };
            (f, cx.pal.text, cx.pal.text_muted)
        };
        let radius = metrics::RADIUS_MD;
        if primary {
            cx.dl.rect(er, radius, fill);
        } else {
            cx.dl.rect_border(er, radius, fill, cx.pal.separator, 1.0);
        }
        let (ir, tx, ty) = if compact {
            let ir = Rect::new(er.x + pad, er.y + (er.h - icon_s) * 0.5, icon_s, icon_s);
            draw_icon(cx.dl, *icon, cx.m_in(ir, er), ink, 1.6, false);
            let tx = er.x + pad * 2.0 + icon_s;
            let lr = cx.m_in(
                Rect::new(tx, er.y + (er.h - lc.height) * 0.5, ctext_w, lc.height),
                er,
            );
            cx.block_top(lc, lr, HAlign::Start, ink);
            if cx.is_focused(&t) {
                cx.button_ring(er, radius);
            }
            cx.hit(er, t);
            continue;
        } else if side {
            (
                Rect::new(er.x + pad, er.y + pad, icon_s, icon_s),
                er.x + pad,
                er.y + pad + icon_s + 10.0,
            )
        } else {
            let text = lb.height + 4.0 + ab.height;
            (
                Rect::new(er.x + pad, er.y + (er.h - icon_s) * 0.5, icon_s, icon_s),
                er.x + pad * 2.0 + icon_s,
                er.y + (er.h - text) * 0.5,
            )
        };
        let ir = cx.m_in(ir, er);
        draw_icon(cx.dl, *icon, ir, ink, 1.6, false);
        let lr = cx.m_in(Rect::new(tx, ty, text_w, lb.height), er);
        cx.block_top(lb, lr, HAlign::Start, ink);
        let ar = cx.m_in(Rect::new(tx, ty + lb.height + 4.0, text_w, ab.height), er);
        cx.block_top(ab, ar, HAlign::Start, muted);
        if cx.is_focused(&t) {
            cx.button_ring(er, radius);
        }
        cx.hit(er, t);
    }
    y += entries_h(compact);
    if !compact {
        y += 10.0;
        let r = cx.m_in(Rect::new(col_x, y, col_w, drop.height), col);
        cx.block_top(&drop, r, HAlign::Start, cx.pal.text_muted);
        y += drop.height;
    }

    // Recent folders.
    if show_recent && n_recent > 0 {
        y += section;
        let hr = cx.m_in(Rect::new(col_x, y, col_w, heading_h), col);
        cx.label(
            &t!("welcome-recent"),
            heading_style,
            hr,
            HAlign::Start,
            cx.pal.text_muted,
        );
        y += heading_h + 10.0;
        for (i, rf) in ch.welcome.recents.iter().take(n_recent).enumerate() {
            let cr = cx.m_in(
                Rect::new(
                    col_x + i as f32 * (card_w + gap),
                    y,
                    card_w,
                    thumb_h + 6.0 + name_h,
                ),
                col,
            );
            let t = Target::Welcome(WelcomeTarget::Recent(i));
            let tr = Rect::new(cr.x, cr.y, card_w, thumb_h);
            if cx.is_hover(&t) {
                cx.dl
                    .rect(cr.inset(-4.0), metrics::RADIUS_MD, cx.pal.surface_hover);
            }
            match rf.thumb {
                Some((tex, w, h)) if w > 0 && h > 0 => {
                    // On a tile, so an image with transparent parts keeps the card's shape.
                    cx.dl.rect_border(
                        tr,
                        metrics::RADIUS_SM,
                        cx.pal.surface_raised,
                        cx.pal.separator,
                        1.0,
                    );
                    // Cropped to fill the card: the middle of a wide image, the upper part of
                    // a tall one (where faces and subjects tend to be).
                    let (iw, ih) = (w as f32, h as f32);
                    let want = tr.w / tr.h;
                    let uv = if iw / ih > want {
                        let u = want * ih / iw;
                        [(1.0 - u) * 0.5, 0.0, u, 1.0]
                    } else {
                        let v = iw / ih / want;
                        [0.0, (1.0 - v) / 3.0, 1.0, v]
                    };
                    cx.dl.image(tr, uv, tex, metrics::RADIUS_SM);
                }
                // No picture in it: a tinted tile, so it does not read as one that failed to
                // load beside a white-backed image.
                _ => {
                    cx.dl.rect_border(
                        tr,
                        metrics::RADIUS_SM,
                        cx.pal.surface,
                        cx.pal.separator,
                        1.0,
                    );
                    let s = cx.u(40.0);
                    draw_icon(
                        cx.dl,
                        Icon::Folder,
                        tr.centered(s, s),
                        cx.pal.text_muted,
                        1.4,
                        false,
                    );
                }
            }
            let nr = Rect::new(cr.x, tr.bottom() + 6.0, card_w, name_h);
            cx.label_fit(&rf.name, name_style, nr, HAlign::Start, cx.pal.text, true);
            if cx.is_focused(&t) {
                cx.focus_ring(cr.inset(-4.0), metrics::RADIUS_MD);
            }
            cx.hit(cr, t);
        }
        y += thumb_h + 6.0 + name_h;
    }

    // Things to try.
    if show_tips && !tips.is_empty() {
        y += section;
        let hr = cx.m_in(Rect::new(col_x, y, col_w, heading_h), col);
        cx.label(
            &t!("welcome-tips"),
            heading_style,
            hr,
            HAlign::Start,
            cx.pal.text_muted,
        );
        // "Hide these tips" at the end of the heading: its text ends where the cards do, and
        // its hover background reaches past that.
        let hide = t!("welcome-tips-hide");
        let hb = cx.layout(&hide, astyle, None);
        let hpad = 8.0;
        let hr = cx.m_in(
            Rect::new(
                col_x + col_w - hb.width - hpad,
                y - 2.0,
                hb.width + hpad * 2.0,
                heading_h + 4.0,
            ),
            col,
        );
        let t = Target::Welcome(WelcomeTarget::HideTips);
        if cx.is_hover(&t) {
            cx.dl.rect(hr, metrics::RADIUS_SM, cx.pal.surface_hover);
        }
        // Darker than the accent: small text needs the contrast.
        let link = cx.pal.accent.lerp(cx.pal.text, 0.2);
        cx.block_in(&hb, hr, HAlign::Center, link);
        cx.ring_if(&t, hr, metrics::RADIUS_SM);
        cx.hit(hr, t);
        y += heading_h + 10.0;
        for (i, (tp, (tb, ab))) in tips.iter().zip(&tip_blocks).enumerate() {
            let (row, c) = (i / cols, i % cols);
            let r = cx.m_in(
                Rect::new(
                    col_x + c as f32 * (tip_w + gap),
                    y + row as f32 * (tip_h + gap),
                    tip_w,
                    tip_h,
                ),
                col,
            );
            let t = Target::Welcome(WelcomeTarget::Tip(i));
            let fill = if cx.is_hover(&t) {
                cx.pal.surface_hover
            } else {
                cx.pal.surface
            };
            cx.dl
                .rect_border(r, metrics::RADIUS_SM + 2.0, fill, cx.pal.separator, 1.0);
            let line = Rect::new(
                r.x + tip_pad,
                r.y + tip_pad,
                tip_text_w,
                tb.height.max(cx.u(20.0)),
            );
            let kw = cx.keycap_end(&tp.key, line, 0.0);
            let title_w = (tip_text_w - kw - 8.0).max(1.0);
            let tr = cx.m_in(Rect::new(line.x, line.y, title_w, line.h), line);
            let title = cx.ellipsize(&tp.kind.title(), tstyle, title_w, false);
            cx.label(&title, tstyle, tr, HAlign::Start, cx.pal.text);
            let ar = Rect::new(r.x + tip_pad, line.bottom() + 4.0, tip_text_w, ab.height);
            cx.block_top(ab, ar, HAlign::Start, cx.pal.text_muted);
            if cx.is_focused(&t) {
                cx.button_ring(r, metrics::RADIUS_SM + 2.0);
            }
            cx.hit(r, t);
        }
    }
    if let Some(b) = &hint {
        y += section;
        let r = cx.m_in(Rect::new(col_x, y, col_w, b.height), col);
        cx.block_top(b, r, HAlign::Start, cx.pal.text_muted);
    }
}

/// While files are dragged over the window: where they go.
fn draw_drop(cx: &mut Ctx) {
    let r = Rect::new(8.0, 8.0, cx.w - 16.0, cx.h - 16.0);
    cx.dl.rect_border(
        r,
        metrics::RADIUS_LG,
        cx.pal.accent.with_alpha(0.08),
        cx.pal.accent,
        2.0,
    );
    let msg = t!("empty-drop-release");
    let style = cx.style(metrics::FONT_TITLE).weight(500.0);
    let b = cx.layout(&msg, style, Some(cx.w - 80.0));
    let cy = cx.h * 0.5 - 20.0;
    let icon = Rect::new(cx.w * 0.5 - 24.0, cy - 64.0, 48.0, 48.0);
    draw_icon(cx.dl, Icon::Image, icon, cx.pal.accent, 1.4, false);
    let br = Rect::new(0.0, cy, cx.w, b.height);
    cx.block_top(&b, br, HAlign::Center, cx.pal.text);
}

/// Every control of the start screen as last drawn, in Tab order.
pub(crate) fn stops(hits: &[(Rect, Target)]) -> Vec<Target> {
    let mut v: Vec<Target> = Vec::new();
    for (_, t) in hits {
        if matches!(t, Target::Welcome(_)) && !v.contains(t) {
            v.push(t.clone());
        }
    }
    // Reading order: the entry points, the recent folders, then the tips' heading link and
    // cards (drawn in that order already).
    v
}
